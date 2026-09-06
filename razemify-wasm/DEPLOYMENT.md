# Cloudflare Pages Deployment Guide

This guide explains how to deploy the Razemify WASM app to Cloudflare Pages using Wrangler.

## Prerequisites

1. **Cloudflare account**: Sign up at [cloudflare.com](https://cloudflare.com)
2. **Wrangler CLI**: Install globally
   ```bash
   npm install -g wrangler
   ```
3. **Authentication**: Log in to Cloudflare
   ```bash
   wrangler login
   ```

## Build the WASM Package

Before deploying, ensure the WASM package is built:

```bash
# From the razemify-wasm directory
bazel run //xtask -- wasm build --release
```

This creates the `pkg/` directory with the compiled WASM files.

## Deploy to Cloudflare Pages

### Option 1: Using the deployment script (Recommended)

```bash
./deploy-to-cloudflare.sh
```

This script:
1. Creates a clean `deploy/` directory
2. Copies all necessary files (HTML, JS, WASM)
3. Deploys to Cloudflare Pages using Wrangler

### Option 2: Manual deployment

```bash
# Prepare deployment directory
mkdir -p deploy
cp www/index.html deploy/
cp www/index.js deploy/
mkdir -p deploy/pkg
cp pkg/razemify_wasm.js deploy/pkg/
cp pkg/razemify_wasm_bg.wasm deploy/pkg/

# Deploy
wrangler pages deploy deploy --project-name=razemify-wasm
```

## Configuration

The `wrangler.toml` file contains deployment configuration:

- **Project name**: `razemify-wasm`
- **Compatibility date**: Set for optimal performance
- **Custom headers**: Required for WASM support
  - `Cross-Origin-Embedder-Policy: require-corp`
  - `Cross-Origin-Opener-Policy: same-origin`
  - Proper MIME type for `.wasm` files

## After Deployment

Your app will be available at:
```
https://razemify-wasm.pages.dev
```

You can also:
- Set up a custom domain in the Cloudflare Pages dashboard
- Configure preview deployments for branches
- Set up automatic deployments from Git (optional)

## Troubleshooting

### WASM fails to load
- Check browser console for CORS errors
- Ensure custom headers are properly configured in `wrangler.toml`
- Verify the WASM file is being served with `application/wasm` MIME type

### Build fails
- Ensure Bazel or Bazelisk is installed and available on `PATH`
- Run `bazel build //razemify-wasm:razemify_wasm` to inspect the Bazel error directly

### Deployment fails
- Verify you're logged in: `wrangler whoami`
- Check project name doesn't conflict with existing projects
- Ensure you have proper permissions in your Cloudflare account

## Updating the Deployment

To update your deployment:

1. Make changes to your code
2. Rebuild the WASM package: `bazel run //xtask -- wasm build --release`
3. Run the deployment script: `./deploy-to-cloudflare.sh`

Each deployment creates a new version, and Cloudflare Pages keeps a history of all deployments.
