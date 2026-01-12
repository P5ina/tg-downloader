# Project: tg-downloader

Telegram bot for downloading YouTube videos.

## Deployment

CI/CD is configured via GitHub Actions. Deploys are triggered automatically on push.

### Branches & Environments

| Branch | Environment | Server Path | Container |
|--------|-------------|-------------|-----------|
| `staging` | Staging (Test Telegram) | `~/projects/tg-downloader-staging` | `tg-downloader-staging-bot` |
| `main` | Production | `~/projects/tg-downloader` | `tg-downloader-bot-1` |

### Deploy Commands

```bash
# Deploy to staging
git checkout staging
git add -A && git commit -m "message"
git push origin staging

# Deploy to production (after testing on staging)
git checkout main
git merge staging --no-edit
git push origin main
```

### Useful Commands

```bash
# Check deploy status
gh run list --limit 3

# Watch deploy in progress
gh run watch

# View logs on server
ssh mafia-game.p5ina.dev "docker logs -f tg-downloader-bot-1"        # production
ssh mafia-game.p5ina.dev "docker logs -f tg-downloader-staging-bot"  # staging

# Restart containers
ssh mafia-game.p5ina.dev "cd ~/projects/tg-downloader && docker compose restart"
ssh mafia-game.p5ina.dev "cd ~/projects/tg-downloader-staging && docker compose restart"
```

## Server

- Host: `mafia-game.p5ina.dev`
- User: `p5ina`
- Both bots share the same `telegram-bot-api` container on port 8081

## Tech Stack

- Rust + Teloxide (Telegram bot framework)
- yt-dlp (YouTube downloads)
- FFmpeg (video processing)
- Docker + Docker Compose
- GitHub Actions (CI/CD)
