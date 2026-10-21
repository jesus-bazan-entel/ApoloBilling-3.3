import type { BucketConfig } from '../types'

/**
 * Default for every NEW plan/account: Fibertel's model has no prepago/
 * postpago or saldo — a plan is Control or Abierto from the moment it's
 * created. Control is the safer default (blocks on exhaustion) since an
 * operator picking defaults shouldn't accidentally create an unmetered
 * Abierto plan.
 */
export const DEFAULT_NEW_BUCKET_CONFIG: BucketConfig = {
  bucket_type: 'control',
  included_minutes: 0,
  proration_mode: 'full',
  overage_flat_rate_per_minute: null,
  overage_tariff_group: 'standard',
}
