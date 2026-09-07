#!/bin/bash

# Build and Deploy Zanbergify WASM to Cloudflare Pages
set -e

echo "🔨 Building WASM package..."
bazel run //xtask -- wasm build --release

echo ""
echo "🚀 Deploying to Cloudflare Pages..."
./deploy-to-cloudflare.sh

echo ""
echo "✅ Build and deployment complete!"
