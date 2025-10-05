#!/bin/bash

set -e

# Configuration
IMAGE_NAME="load-tester"
IMAGE_TAG="latest"
NAMESPACE="load-testing"
AWS_REGION="us-east-1"
AWS_ACCOUNT_ID="${AWS_ACCOUNT_ID:-$(aws sts get-caller-identity --query Account --output text)}"
USE_ECR="true"  # Set to "false" if not using ECR

echo "🚀 Building and deploying load tester to EKS..."

# Step 1: Build the Docker image
echo "📦 Building Docker image..."
docker buildx build -t ${IMAGE_NAME}:${IMAGE_TAG} . --platform=linux/amd64

# Step 2: Tag for ECR (optional - if using ECR)
if [ "$USE_ECR" = "true" ]; then
    ECR_REGISTRY="${AWS_ACCOUNT_ID}.dkr.ecr.${AWS_REGION}.amazonaws.com"
    aws ecr create-repository --repository-name ${IMAGE_NAME} 2>/dev/null
    ECR_REPO="${ECR_REGISTRY}/${IMAGE_NAME}"

    echo "🏷️  Tagging for ECR..."
    docker tag ${IMAGE_NAME}:${IMAGE_TAG} ${ECR_REPO}:${IMAGE_TAG}

    echo "🔐 Logging into ECR..."
    aws ecr get-login-password --region ${AWS_REGION} | docker login --username AWS --password-stdin ${ECR_REGISTRY}

    echo "⬆️  Pushing to ECR..."
    docker push ${ECR_REPO}:${IMAGE_TAG}
fi
