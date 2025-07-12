use anyhow::Result;
use aws_config::BehaviorVersion;
use aws_sdk_s3::Client;
use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "lmbcli")]
#[command(about = "A CLI for listing and downloading files from S3 buckets")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// List files within a date range
    List {
        /// Start date in RFC3339 format (e.g., 2025-12-15T03:00:00Z)
        #[arg(long)]
        from: DateTime<Utc>,
        /// End date in RFC3339 format (e.g., 2025-12-15T04:00:00Z)
        #[arg(long)]
        to: DateTime<Utc>,
        /// S3 bucket name
        #[arg(long)]
        bucket: String,
    },
    /// Download a specific file
    Download {
        /// File key in timestamp format (e.g., 20251215T033000Z-errors.log) or S3 path
        #[arg(long)]
        key: String,
        /// S3 bucket name
        #[arg(long)]
        bucket: String,
        /// Output file path
        #[arg(long)]
        output: PathBuf,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize AWS configuration
    let config = aws_config::defaults(BehaviorVersion::latest()).load().await;
    let s3_client = Client::new(&config);

    match cli.command {
        Commands::List { from, to, bucket } => {
            list_files(&s3_client, &bucket, from, to).await?;
        }
        Commands::Download { key, bucket, output } => {
            download_file(&s3_client, &bucket, &key, &output).await?;
        }
    }

    Ok(())
}

/// List files in S3 bucket within date range
async fn list_files(
    client: &Client,
    bucket: &str,
    from: DateTime<Utc>,
    to: DateTime<Utc>,
) -> Result<()> {
    use aws_sdk_s3::operation::list_objects_v2::ListObjectsV2Output;
    
    let from_year = from.format("%Y").to_string();
    
    // Start with the from year as prefix to narrow down the search
    let prefix = from_year;
    
    let mut continuation_token: Option<String> = None;
    
    loop {
        let mut request = client
            .list_objects_v2()
            .bucket(bucket)
            .prefix(&prefix);
        
        if let Some(token) = continuation_token {
            request = request.continuation_token(token);
        }
        
        let response: ListObjectsV2Output = request.send().await?;
        
        if let Some(objects) = response.contents {
            for object in objects {
                if let Some(key) = object.key {
                    if let Some(last_modified) = object.last_modified {
                        // Convert AWS DateTime to chrono DateTime
                        let last_modified = DateTime::<Utc>::from(
                            std::time::SystemTime::try_from(last_modified)?
                        );
                        
                        // Filter by date range
                        if last_modified >= from && last_modified <= to {
                            // Convert S3 path to output format
                            if let Some(formatted_key) = format_s3_key_to_output(&key) {
                                println!("{}", formatted_key);
                            }
                        }
                    }
                }
            }
        }
        
        // Check if there are more results
        if response.is_truncated == Some(true) {
            continuation_token = response.next_continuation_token;
        } else {
            break;
        }
    }
    
    Ok(())
}

/// Download a file from S3
async fn download_file(
    client: &Client,
    bucket: &str,
    key: &str,
    output_path: &PathBuf,
) -> Result<()> {
    use tokio::io::AsyncReadExt;
    
    // Convert timestamp format to S3 path if needed
    let s3_key = if is_timestamp_format(key) {
        convert_timestamp_to_s3_path(key)?
    } else {
        key.to_string()
    };
    
    let response = client
        .get_object()
        .bucket(bucket)
        .key(&s3_key)
        .send()
        .await?;
    
    let mut body = response.body.into_async_read();
    let mut buffer = Vec::new();
    body.read_to_end(&mut buffer).await?;
    
    std::fs::write(output_path, buffer)?;
    println!("Downloaded {} to {}", s3_key, output_path.display());
    
    Ok(())
}

/// Convert S3 key path to output format
/// From: 2025/12/15/T/03/30/errors.log
/// To: 20251215T033000Z-errors.log
fn format_s3_key_to_output(s3_key: &str) -> Option<String> {
    let parts: Vec<&str> = s3_key.split('/').collect();
    
    // Expected format: YYYY/MM/DD/T/HH/MM/filename.ext
    if parts.len() >= 7 {
        let year = parts[0];
        let month = parts[1];
        let day = parts[2];
        let t_marker = parts[3];
        let hour = parts[4];
        let minute = parts[5];
        let filename = parts[6..].join("/"); // Handle filenames with slashes
        
        if t_marker == "T" && year.len() == 4 && month.len() == 2 && day.len() == 2 
            && hour.len() == 2 && minute.len() == 2 {
            return Some(format!("{}{}{}T{}{}00Z-{}", year, month, day, hour, minute, filename));
        }
    }
    
    None
}

/// Check if a key is in timestamp format
fn is_timestamp_format(key: &str) -> bool {
    // Format: 20251215T033000Z-filename.ext
    if let Some(dash_pos) = key.find('-') {
        let timestamp_part = &key[..dash_pos];
        return timestamp_part.len() == 16 
            && timestamp_part.chars().nth(8) == Some('T')
            && timestamp_part.chars().nth(15) == Some('Z')
            && timestamp_part[..8].chars().all(|c| c.is_ascii_digit())
            && timestamp_part[9..15].chars().all(|c| c.is_ascii_digit());
    }
    false
}

/// Convert timestamp format to S3 path
/// From: 20251215T033000Z-errors.log
/// To: 2025/12/15/T/03/30/errors.log
fn convert_timestamp_to_s3_path(timestamp_key: &str) -> Result<String> {
    if let Some(dash_pos) = timestamp_key.find('-') {
        let timestamp_part = &timestamp_key[..dash_pos];
        let filename = &timestamp_key[dash_pos + 1..];
        
        if timestamp_part.len() == 16 {
            let year = &timestamp_part[0..4];
            let month = &timestamp_part[4..6];
            let day = &timestamp_part[6..8];
            let hour = &timestamp_part[9..11];
            let minute = &timestamp_part[11..13];
            
            return Ok(format!("{}/{}/{}/T/{}/{}/{}", year, month, day, hour, minute, filename));
        }
    }
    
    anyhow::bail!("Invalid timestamp format: {}", timestamp_key);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_s3_key_to_output() {
        // Test valid S3 key conversion
        let s3_key = "2025/12/15/T/03/30/errors.log";
        let expected = "20251215T033000Z-errors.log";
        assert_eq!(format_s3_key_to_output(s3_key), Some(expected.to_string()));

        // Test with filename containing slashes
        let s3_key = "2025/12/15/T/03/30/logs/app/errors.log";
        let expected = "20251215T033000Z-logs/app/errors.log";
        assert_eq!(format_s3_key_to_output(s3_key), Some(expected.to_string()));

        // Test invalid format
        let s3_key = "invalid/path";
        assert_eq!(format_s3_key_to_output(s3_key), None);
    }

    #[test]
    fn test_is_timestamp_format() {
        // Test valid timestamp format
        assert!(is_timestamp_format("20251215T033000Z-errors.log"));
        
        // Test invalid formats
        assert!(!is_timestamp_format("errors.log"));
        assert!(!is_timestamp_format("20251215-errors.log"));
        assert!(!is_timestamp_format("2025/12/15/T/03/30/errors.log"));
    }

    #[test]
    fn test_convert_timestamp_to_s3_path() {
        // Test valid timestamp conversion
        let timestamp_key = "20251215T033000Z-errors.log";
        let expected = "2025/12/15/T/03/30/errors.log";
        assert_eq!(convert_timestamp_to_s3_path(timestamp_key).unwrap(), expected);

        // Test with complex filename
        let timestamp_key = "20251215T033000Z-logs/app/errors.log";
        let expected = "2025/12/15/T/03/30/logs/app/errors.log";
        assert_eq!(convert_timestamp_to_s3_path(timestamp_key).unwrap(), expected);

        // Test invalid format
        assert!(convert_timestamp_to_s3_path("invalid-format").is_err());
    }
}
