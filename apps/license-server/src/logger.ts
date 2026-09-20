// Structured logging with ip hashing (docs/13 § 7 — no PII beyond device_name).
import pino from 'pino';

export const logger = pino({
  level: process.env.LOG_LEVEL ?? 'info',
  redact: { paths: ['req.headers.authorization'], censor: '[redacted]' },
});
