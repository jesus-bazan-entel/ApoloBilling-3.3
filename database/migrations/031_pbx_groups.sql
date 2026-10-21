-- Migration 031: PBX Groups (same-owner failover sets)
-- Date: 2026-10-03
-- Description: Grupos de PBXs del MISMO dueno (primaria/secundaria), espejo de
--   Carrier Groups pero para troncales IP. El L3 selectivo puentea cada rango
--   a los miembros del grupo en orden de prioridad (failover legitimo: mismo
--   cliente). Un endpoint pertenece como maximo a UN grupo
--   (UNIQUE sip_endpoint_id): los rangos (did_ranges) se heredan del grupo.

BEGIN;

CREATE TABLE IF NOT EXISTS pbx_groups (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(100) NOT NULL UNIQUE,
    description TEXT,
    strategy VARCHAR(20) NOT NULL DEFAULT 'sequential'
        CHECK (strategy IN ('sequential', 'priority')),
    enabled BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS pbx_group_members (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    group_id UUID NOT NULL REFERENCES pbx_groups(id) ON DELETE CASCADE,
    sip_endpoint_id UUID NOT NULL REFERENCES sip_endpoints(id) ON DELETE CASCADE,
    priority INTEGER NOT NULL DEFAULT 100,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    CONSTRAINT uq_pbx_group_member UNIQUE (group_id, sip_endpoint_id)
);

-- Un endpoint, un grupo: los rangos de un grupo pertenecen a un solo dueno.
CREATE UNIQUE INDEX IF NOT EXISTS uq_pbx_group_member_endpoint
    ON pbx_group_members (sip_endpoint_id);

CREATE INDEX IF NOT EXISTS idx_pbx_group_members_group
    ON pbx_group_members (group_id);

COMMIT;
