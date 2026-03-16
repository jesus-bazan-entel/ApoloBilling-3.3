# Análisis del Modelo de Base de Datos - Zonas, Prefijos y Tarifas

## Estado Actual

### 🔴 Problema: Duplicación de Modelos

Actualmente existen **DOS modelos diferentes** para manejar tarifas:

#### 1. **Modelo Python Backend** (zones.py)
```
zones (Zone)
├── id
├── zone_name
├── zone_code
└── description

prefixes (Prefix)
├── id
├── zone_id → zones.id
├── prefix
└── prefix_length

rate_zones (RateZone)
├── id
├── zone_id → zones.id
├── rate_per_minute
└── billing_increment
```

**Uso:** Sistema legacy/administrativo, NO usado por el motor Rust

#### 2. **Modelo Rust Engine** (rate.rs + billing.py)
```
rate_cards (RateCard)
├── id
├── destination_prefix
├── destination_name
├── rate_per_minute
├── billing_increment
├── connection_fee
├── effective_start
├── effective_end
└── priority
```

**Uso:** Motor de billing en tiempo real (Rust) y servicio de rating (Python)

---

## 🎯 Recomendación: **Unificar en `rate_cards`**

### ✅ Razones:

1. **Ya usado por el motor crítico (Rust)**
   - El motor de billing en Rust lee directamente de `rate_cards`
   - Es el path crítico de autorización de llamadas
   - Alto rendimiento, ya optimizado

2. **Modelo más simple y eficiente**
   - Tabla única con Longest Prefix Match
   - Sin JOIN necesario (zona + prefijo + tarifa)
   - Menos latencia en queries críticos

3. **Funcionalidad completa**
   - `destination_prefix`: Soporta LPM (1, 52, 5491, etc.)
   - `destination_name`: Nombre descriptivo (equivale a zone_name)
   - `effective_start/end`: Vigencia temporal de tarifas
   - `priority`: Resolución de conflictos
   - `billing_increment`: Redondeo por tarifa

4. **Usado por ambos sistemas**
   - `rust-billing-engine`: Autorización y CDR
   - `backend/services/rating.py`: Cálculo de tarifas

### ❌ Problemas del modelo `zones/prefixes/rate_zones`:

1. **NO usado por el motor Rust** (el componente crítico)
2. **Requiere 3 tablas y 2 JOINs** para obtener una tarifa
3. **Menos flexible** para patrones de prefijos complejos
4. **Duplicación de datos** si se mantienen ambos

---

## 📋 Estrategia de Migración

### Fase 1: Consolidación (INMEDIATO)

1. **Migrar datos de `zones/prefixes/rate_zones` → `rate_cards`**
   ```sql
   INSERT INTO rate_cards (
     destination_prefix, 
     destination_name, 
     rate_per_minute, 
     billing_increment,
     priority
   )
   SELECT 
     p.prefix,
     z.zone_name || ' - ' || z.description,
     rz.rate_per_minute * 60, -- convertir de por segundo a por minuto
     rz.billing_increment,
     rz.priority
   FROM prefixes p
   JOIN zones z ON p.zone_id = z.id
   JOIN rate_zones rz ON rz.zone_id = z.id
   WHERE rz.enabled = true AND p.enabled = true;
   ```

2. **Actualizar backend Python para usar solo `rate_cards`**
   - Ya está parcialmente implementado en `rating.py`
   - Eliminar dependencias de `Zone`, `Prefix`, `RateZone`

3. **Deprecar tablas antiguas**
   - Mantener por 1-2 versiones para rollback
   - Agregar triggers de advertencia
   - Documentar como deprecated

### Fase 2: UI Administrativa (CORTO PLAZO)

1. **Actualizar UI de gestión de tarifas**
   - Formularios para CRUD de `rate_cards`
   - Importación masiva de prefijos
   - Validación de overlapping prefixes

2. **Herramientas de gestión**
   - Script de validación de consistencia
   - Export/import CSV de rate_cards
   - Comparador de tarifas (histórico)

### Fase 3: Limpieza (MEDIANO PLAZO)

1. **Eliminar tablas antiguas**
   ```sql
   DROP TABLE rate_zones;
   DROP TABLE prefixes;
   DROP TABLE zones;
   ```

2. **Limpiar código**
   - Eliminar modelos `Zone`, `Prefix`, `RateZone`
   - Eliminar imports no usados
   - Actualizar documentación

---

## 🛠️ Schema Final Recomendado

### Tabla Principal: `rate_cards`

```sql
CREATE TABLE rate_cards (
    id SERIAL PRIMARY KEY,
    
    -- Identificación
    destination_prefix VARCHAR NOT NULL,  -- '1', '52', '5491', '549115'
    destination_name VARCHAR NOT NULL,     -- 'USA', 'Mexico', 'Argentina Mobile'
    
    -- Tarifas
    rate_per_minute NUMERIC(10, 4) NOT NULL,
    billing_increment INTEGER DEFAULT 60,  -- seconds
    connection_fee NUMERIC(10, 4) DEFAULT 0.0000,
    
    -- Vigencia
    effective_start TIMESTAMP DEFAULT NOW(),
    effective_end TIMESTAMP NULL,
    
    -- Control
    priority INTEGER DEFAULT 0,            -- Mayor = más prioritario
    enabled BOOLEAN DEFAULT true,
    
    -- Metadata
    created_at TIMESTAMP DEFAULT NOW(),
    updated_at TIMESTAMP DEFAULT NOW(),
    
    -- Indices
    CONSTRAINT unique_prefix_period UNIQUE (destination_prefix, effective_start)
);

CREATE INDEX idx_rate_cards_prefix ON rate_cards(destination_prefix);
CREATE INDEX idx_rate_cards_effective ON rate_cards(effective_start, effective_end);
CREATE INDEX idx_rate_cards_enabled ON rate_cards(enabled) WHERE enabled = true;
```

### Ventajas del Diseño:

1. **Longest Prefix Match eficiente**
   ```sql
   -- Query del motor Rust (ya implementado)
   SELECT * FROM rate_cards
   WHERE destination_prefix = ANY($prefixes)  -- ['549115551', '54911555', ...]
     AND effective_start <= NOW()
     AND (effective_end IS NULL OR effective_end >= NOW())
     AND enabled = true
   ORDER BY LENGTH(destination_prefix) DESC, priority DESC
   LIMIT 1;
   ```

2. **Versionado temporal de tarifas**
   - Múltiples registros con mismo prefix
   - Diferentes `effective_start/end`
   - CDRs históricos mantienen consistencia

3. **Resolución de conflictos**
   - `priority` para overlapping prefixes
   - Ejemplo: '1' (genérico) vs '1800' (toll-free)

4. **Metadatos flexibles** (futuro)
   - Agregar columnas: `carrier`, `route_type`, `quality_tier`
   - JSON field para atributos custom

---

## 📊 Comparación de Rendimiento

### Modelo Anterior (3 tablas):
```sql
SELECT rz.rate_per_minute, rz.billing_increment
FROM prefixes p
JOIN zones z ON p.zone_id = z.id
JOIN rate_zones rz ON rz.zone_id = z.id
WHERE p.prefix = '5491'  -- Requiere match exacto
LIMIT 1;
```
- **3 tablas, 2 JOINs**
- **NO soporta LPM nativo**
- **~5-10ms latencia**

### Modelo Nuevo (1 tabla):
```sql
SELECT rate_per_minute, billing_increment
FROM rate_cards
WHERE destination_prefix = ANY($prefixes)
  AND effective_start <= NOW()
  AND (effective_end IS NULL OR effective_end >= NOW())
ORDER BY LENGTH(destination_prefix) DESC
LIMIT 1;
```
- **1 tabla, 0 JOINs**
- **LPM nativo con ANY + ORDER BY LENGTH**
- **~1-2ms latencia**
- **Cache-friendly (Redis)**

---

## 🚀 Acción Inmediata

**DECISIÓN:** Usar exclusivamente `rate_cards` como fuente única de verdad.

**Próximos pasos:**
1. ✅ Documentar decisión (este archivo)
2. ⏭️ Crear script de migración de datos
3. ⏭️ Actualizar UI administrativa
4. ⏭️ Deprecar modelos antiguos
5. ⏭️ Actualizar documentación de API

---

## 📝 Notas Adicionales

### Compatibilidad con sistemas legacy:
Si existen integraciones que dependen de `zones`:
- Crear **VIEWs SQL** que emulen el modelo antiguo
- Mapear `destination_name` → `zone_name`
- Agrupar prefijos por patrón

### Performance tips:
- **Particionar** `rate_cards` por `effective_start` si crece mucho
- **Cache** en Redis: `rate:{prefix}` → TTL 5 minutos
- **Materialized view** para analytics

### Auditabilidad:
- NO eliminar registros, usar `enabled = false`
- Mantener histórico completo con `effective_end`
- Logs de cambios en tabla separada si es necesario
