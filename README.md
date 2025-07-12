# lmbcli

A Rust CLI application for listing and downloading files from AWS S3 buckets.

## Features

- **List files within a date range**: Uses AWS S3 ListObjectsV2 API to list files with date range filtering
- **Download files**: Download files from S3 bucket using timestamp-key format or S3 path
- Supports S3 bucket files stored with prefix structure: `YYYY/MM/DD/T/HH/MM/filename.ext`
- Outputs files in format: `YYYYMMDDTHHMMSSZ-filename.ext`

## Installation

Build from source:

```bash
git clone <repository>
cd lmbcli
cargo build --release
```

The binary will be available at `target/release/lmbcli`.

## Prerequisites

- AWS credentials configured (via AWS CLI, environment variables, or IAM roles)
- AWS region configured (via `AWS_REGION` environment variable or AWS configuration)

## Usage

### List files in date range

```bash
lmbcli list --from 2025-12-15T03:00:00Z --to 2025-12-15T04:00:00Z --bucket my-bucket
```

Example output:
```
20251215T033000Z-errors.log
20251215T033500Z-access.log
20251215T034000Z-debug.log
```

### Download a specific file

Using timestamp format:
```bash
lmbcli download --key 20251215T033000Z-errors.log --bucket my-bucket --output ./errors.log
```

Using S3 path format:
```bash
lmbcli download --key 2025/12/15/T/03/30/errors.log --bucket my-bucket --output ./errors.log
```

## File Structure

The CLI expects S3 bucket files to be stored with the following structure:
```
bucket/
├── 2025/
│   └── 12/
│       └── 15/
│           └── T/
│               ├── 03/
│               │   ├── 30/
│               │   │   └── errors.log
│               │   └── 35/
│               │       └── access.log
│               └── 04/
│                   └── 00/
│                       └── debug.log
```

The CLI converts these paths to the output format:
- `2025/12/15/T/03/30/errors.log` → `20251215T033000Z-errors.log`

## Configuration

### AWS Credentials

Configure your AWS credentials using one of these methods:

1. AWS CLI: `aws configure`
2. Environment variables:
   ```bash
   export AWS_ACCESS_KEY_ID=your_access_key
   export AWS_SECRET_ACCESS_KEY=your_secret_key
   ```
3. IAM roles (when running on EC2)

### AWS Region

Set the AWS region:
```bash
export AWS_REGION=us-east-1
```

If no region is specified, the CLI defaults to `us-east-1`.

## Error Handling

The CLI provides detailed error messages for common issues:

- Invalid date format
- Missing AWS credentials or region
- S3 bucket access errors
- File download errors
- Invalid timestamp formats

## Examples

### List all files from a specific day
```bash
lmbcli list --from 2025-12-15T00:00:00Z --to 2025-12-15T23:59:59Z --bucket my-logs
```

### Download a file to a specific directory
```bash
lmbcli download --key 20251215T033000Z-app.log --bucket my-logs --output ./logs/app.log
```

The CLI will automatically create the output directory if it doesn't exist.

## Technical Details

- Built with Rust using the AWS SDK for Rust
- Uses `clap` for CLI argument parsing
- Supports proper S3 pagination for large bucket listings
- Handles AWS SDK's async operations with `tokio`
- Provides comprehensive error handling with `anyhow`

## License

[Add your license information here]