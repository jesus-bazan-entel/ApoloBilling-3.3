-- Migration 029: DID ranges live on the PBX endpoint (single source of truth)
-- Date: 2026-10-03
-- Description: El rango de numeracion/DIDs de cada cliente se configura EN EL
--   PROPIO endpoint trunk/ip (columna did_ranges, prefijos separados por coma,
--   ej: '174020,174021'), no en una tabla separada. Reemplaza pbx_did_ranges
--   (028, vacia, se elimina): una sola fuente de verdad junto a host/puerto.
--   El L3 selectivo lee did_ranges y publica una extension por prefijo
--   (longest-match) solo hacia su PBX duena.

BEGIN;

ALTER TABLE sip_endpoints
    ADD COLUMN IF NOT EXISTS did_ranges TEXT NOT NULL DEFAULT '';

COMMENT ON COLUMN sip_endpoints.did_ranges IS
'Rangos DID propiedad de esta PBX (solo trunk/ip): prefijos separados por coma, ej ''174020,174021''. El L3 selectivo enruta cada prefijo SOLO a esta PBX (longest-match). Vacio = sin rangos asignados.';

DROP TABLE IF EXISTS pbx_did_ranges;

COMMIT;
