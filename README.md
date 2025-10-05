# Microservice Load Tester (Rust)

A blazingly fast, high-concurrent CLI load testing tool for microservices with AWS SSM endpoint discovery.

## Features

- 🚀 **Ultra-high concurrent load testing** with Rust's async performance
- 🔍 **Automatic endpoint discovery** via AWS SSM
- 📊 **Detailed performance metrics** and real-time reporting
- 🎯 **Realistic user scenarios** with complex workflows
- 🛡️ **Built-in timeouts** and error handling

## Installation

```bash
cargo build --release
```

This won't probably not work on a local machine as while it can discover
the endpoints through SSM, it won't be able to reach to the internal LBs.

You'll need to run it either on a EC2 instance on the workshop VPC or
from EKS. Make sure to give SSM Parameter store permissions to the EC2 instance,
or EKS node groups

Quick start example from EKS:

```
./build-ecr.sh
k run --rm -it --attach petfood-test --image {ACCOUNT_ID}.dkr.ecr.{REGION}.amazonaws.com/load-tester -n default --command --  /app/load-tester --users 10 --concurrent 2
```

Long running test on EKS

```
k run --rm -it --attach petfood-test --image {ACCOUNT_ID}.dkr.ecr.{REGION}.amazonaws.com/load-tester -n default --command -- /bin/sh
$ while true; do /app/load-tester --users $(shuf -i2-10 -n1) --concurrent 2 --verbose; done 
```

you can make concurrency and users more random and more dynamic. Read the usage below

## Usage

### Basic Usage
```bash
cargo run --release
```

### Custom Configuration
```bash
# Test with 20 users, 10 concurrent requests each
cargo run --release -- --users 20 --concurrent 10

# Gradual ramp-up over 30 seconds (realistic load testing)
cargo run --release -- --users 50 --concurrent 20 --rampup 30

# Specify AWS region
cargo run --release -- --region us-west-2

# Verbose output showing individual requests
cargo run --release -- --users 10 --concurrent 5 --verbose

# Dry run (show what would be tested)
cargo run --release -- --dry-run

# Build and run the binary directly
cargo build --release
./target/release/load-tester --users 50 --concurrent 20 --rampup 60
```

### Command Line Options

- `-u, --users <number>`: Number of concurrent users (default: 10)
- `-c, --concurrent <number>`: Concurrent requests per user (default: 5)  
- `-r, --region <region>`: AWS region for SSM discovery (default: us-east-1)
- `--rampup <seconds>`: Gradually increase load over time (default: 0 = immediate)
- `--dry-run`: Show what would be tested without executing
- `-v, --verbose`: Show detailed breakdown and individual request results

## Test Scenario

Each user runs through this comprehensive pet adoption and shopping scenario (20 requests total):

### Pet Search & Discovery (4 requests)
1. **List All Pets** - GET `/api/search` (discover available pets)
2. **Filter by Color** - GET `/api/search?petcolor=black|brown|purple|red|blue` (random color)
3. **Filter by Type** - GET `/api/search?pettype=puppy|kitten|bunny` (random type)
4. **Search for Puppies** - GET `/api/search?pettype=puppy` (find puppies to adopt)
5. **Search for Kittens** - GET `/api/search?pettype=kitten` (find kittens to adopt)  
6. **Search for Bunnies** - GET `/api/search?pettype=bunny` (find bunnies to adopt)

### Triple Pet Adoption (4 requests)
7. **Adopt a Puppy** - POST `/api/completeadoption?petId=xxx&petType=puppy&userId=xxx`
8. **Adopt a Kitten** - POST `/api/completeadoption?petId=xxx&petType=kitten&userId=xxx`
9. **Adopt a Bunny** - POST `/api/completeadoption?petId=xxx&petType=bunny&userId=xxx`
10. **Verify Adoptions** - GET `/api/adoptionlist` (check all adoptions were recorded)

### Comprehensive Pet Food Testing (8 requests)
11. **List All Foods** - GET `/api/foods`
12. **Filter Foods** - GET `/api/foods?pettype=puppy&max_price=10` (random filters)
13. **Search Foods** - GET `/api/foods?search=royal` (random search terms)
14. **Get Food Details** - GET `/api/foods/{foodId}` (specific food item)
15. **List Cart** - GET `/api/cart/{userId}` (current cart contents)
16. **Add to Cart** - POST `/api/cart/{userId}/items` (add food item)
17. **Update Cart** - PUT `/api/cart/{userId}/items/{foodId}` (change quantity)
18. **Checkout** - POST `/api/cart/{userId}/checkout` (complete purchase)

### Cleanup (2 requests)
19. **Empty Cart** - DELETE `/api/cart/{userId}` (clear cart)
20. **Bulk Cleanup Adoptions** - DELETE `/api/cleanupadoptions/{userId}` (cleanup all user adoptions in one call)

### Efficient API Design
- **Total**: 20 requests per user scenario
- **Bulk Operations**: Single cleanup call instead of multiple individual cleanups
- **Realistic Patterns**: Mirrors real-world API usage with bulk operations
- **Performance**: Reduces cleanup overhead and network round-trips

### API Response Format
The search API returns an array of pet objects:
```json
[
  {
    "petid": "022",
    "availability": "yes",
    "cuteness_rate": "5",
    "petcolor": "black",
    "pettype": "kitten",
    "price": "75",
    "peturl": "https://..."
  }
]
```

### Food API Features Tested
- **Filtering**: By pet type, price range
- **Search**: Text-based food search
- **Cart Management**: Add, update, list, checkout, empty
- **Payment Processing**: Full checkout with credit card and addresses

## Ramp-Up Load Testing

The `--rampup` flag enables realistic load testing by gradually increasing concurrent load over time instead of hitting services with full load immediately.

### Benefits of Ramp-Up
- **Realistic Traffic Patterns**: Mimics real-world user behavior
- **Service Warm-Up**: Allows services to scale and warm up gradually
- **Better Error Detection**: Identifies breaking points more accurately
- **Reduced False Positives**: Avoids overwhelming cold services

### Examples
```bash
# Gentle ramp-up: 100 scenarios over 60 seconds
./target/release/load-tester --users 20 --concurrent 5 --rampup 60

# Aggressive ramp-up: 500 scenarios over 30 seconds  
./target/release/load-tester --users 50 --concurrent 10 --rampup 30

# Immediate load (traditional approach)
./target/release/load-tester --users 50 --concurrent 10 --rampup 0
```

### Ramp-Up Calculation
- **Total Scenarios**: `users × concurrent`
- **Interval**: `rampup_seconds ÷ total_scenarios`
- **Pattern**: New scenario starts every interval until all are running

## Real-Time Progress Monitoring

All tests now show live progress updates during execution, providing immediate feedback on test status.

### Default Output (Clean & Focused)
```bash
🚀 Microservice Load Tester
🔍 Discovering endpoints from SSM...
✓ Found petsearch: http://internal-lb-petsearch.com/api/search
📈 Ramp-up Progress: 25.0% (250/1000) - 30s elapsed
📊 Execution Progress: 45.2% (452/1000) | 9040 requests (18 failed) | 225.3 req/s | 80s elapsed
✅ All scenarios completed: 1000/1000 | 20000 total requests (19982 successful, 18 failed)

📊 Load Test Results
══════════════════════════════════════════════════
Total Scenarios: 1000
Total Requests: 20000
✓ Successful: 19982
✗ Failed: 18
Success Rate: 99.9%
Average Response Time: 245ms
Requests/Second: 187.5
Total Test Time: 80000ms
══════════════════════════════════════════════════
```

### Verbose Output (Detailed Analysis)
Use `--verbose` flag to see:
- **Individual request logs** during execution
- **Detailed error information** for failed requests
- **Per-endpoint breakdown** with success rates
- **Failed scenario summaries**

### Progress Update Frequency
- **Small tests** (≤20 scenarios): Every 2 seconds
- **Medium tests** (21-100 scenarios): Every 5 seconds  
- **Large tests** (>100 scenarios): Every 10 seconds

## Endpoint Discovery

The tool automatically discovers endpoints from AWS SSM parameters:

- `/microservices/petlistadoptions/endpoint`
- `/microservices/petsearch/endpoint`
- `/microservices/payforadoption/endpoint`
- `/microservices/petfood/endpoint`

### Fallback Endpoints

If SSM discovery fails, these fallback endpoints are used:

- `PETLIST_ENDPOINT` (default: http://localhost:8080)
- `PETSEARCH_ENDPOINT` (default: http://localhost:8081)
- `PAYFORADOPTION_ENDPOINT` (default: http://localhost:8082)
- `PETFOOD_ENDPOINT` (default: http://localhost:8083)

## Environment Variables

```bash
export PETLIST_ENDPOINT=https://your-petlist-service.com
export PETSEARCH_ENDPOINT=https://your-petsearch-service.com
export PAYFORADOPTION_ENDPOINT=https://your-payforadoption-service.com
export PETFOOD_ENDPOINT=https://your-petfood-service.com
```

## AWS Configuration

Ensure your AWS credentials are configured:

```bash
aws configure
# or use environment variables
export AWS_ACCESS_KEY_ID=your-key
export AWS_SECRET_ACCESS_KEY=your-secret
export AWS_DEFAULT_REGION=us-east-1
```

## Example Output

```
🚀 Microservice Load Tester
Users: 10, Concurrent: 5, Region: us-east-1

🔍 Discovering endpoints from SSM...
✓ Found petlistadoptions: https://api.example.com
✓ Found petsearch: https://search.example.com
⚠️  Parameter not found: /microservices/payforadoption/endpoint
🔄 Using fallback for payforadoption: http://localhost:8082

🎯 Starting load test...
⚡ Running 50 concurrent scenarios...

📊 Load Test Results
══════════════════════════════════════════════════
Total Scenarios: 50
Total Requests: 1000
✓ Successful: 980
✗ Failed: 20
Success Rate: 98%
Average Response Time: 245ms
Requests/Second: 187
Total Test Time: 5340ms
══════════════════════════════════════════════════
```
