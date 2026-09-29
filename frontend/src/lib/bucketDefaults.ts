import type { BucketConfig } from '../types'

/**
 * Legacy balance-model default. Only used to seed the bucket-config editor
 * for pre-existing plans/accounts that still have `billing_model='balance'`
 * (e.g. the old prepago/postpago records) — never used for anything new.
 */
export const DEFAULT_BUCKET_CONFIG: BucketConfig = {
  billing_model: 'balance',
  bucket_type: null,
  included_minutes: 0,
  proration_mode: 'full',
  overage_flat_rate_per_minute: null,
  overage_tariff_group: 'standard',
}

/**
 * Default for every NEW plan/account: Fibertel's model has no prepago/
 * postpago or saldo — a plan is Control or Abierto from the moment it's
 * created. Control is the safer default (blocks on exhaustion) since an
 * operator picking defaults shouldn't accidentally create an unmetered
 * Abierto plan.
 */
export const DEFAULT_NEW_BUCKET_CONFIG: BucketConfig = {
  ...DEFAULT_BUCKET_CONFIG,
  billing_model: 'minute_bucket',
  bucket_type: 'control',
}
