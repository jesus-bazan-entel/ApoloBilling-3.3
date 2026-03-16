from sqlalchemy.orm import Session
from sqlalchemy import func
from app.models.billing import RateCard

def determinar_zona_y_tarifa(numero_marcado: str, db: Session):
    """
    Determina la zona y tarifa usando Longest Prefix Match (LPM) contra RateCard.
    """
    # Limpiar el número (quitar caracteres especiales)
    numero_limpio = ''.join(filter(str.isdigit, numero_marcado))
    
    if not numero_limpio:
        return {
            'prefijo_id': None,
            'zona_id': None,
            'prefijo': 'EMPTY',
            'zona_nombre': 'Desconocida',
            'zona_descripcion': 'Número vacío',
            'tarifa_segundo': 0.0,
            'tarifa_id': None,
            'numero_valido': False
        }

    # Estrategia LPM:
    # 1. Buscar todos los RateCards cuyo prefijo sea un prefijo del numero_limpio
    #    Para esto, en SQL puro sería: WHERE :numero_limpio LIKE destination_prefix || '%'
    #    En python/sqlalchemy es más facil traer candidatos o iterar. 
    #    Dado que destination_prefix es variable, podemos buscar RateCards donde el numero empiece con el prefijo.
    
    # Eficiente en SQL: length(destination_prefix) <= length(numero_limpio) AND :numero_limpio LIKE destination_prefix || '%'
    # Sin embargo, 'LIKE' al revés no es estandar.
    # Alternativa común: traer todos los rate cards y filtrar en python (lento si son muchos).
    # Alternativa mejor: Buscar coincidencia exacta de substrings.
    # SELECT * FROM rate_cards WHERE destination_prefix IN (substring(num, 1, 1), substring(num, 1, 2), ...)
    
    # Generar todos los posibles prefijos del número marcado
    posibles_prefijos = [numero_limpio[:i] for i in range(1, len(numero_limpio) + 1)]
    
    # Buscar RateCards que coincidan con alguno de estos prefijos
    matches = db.query(RateCard).filter(
        RateCard.destination_prefix.in_(posibles_prefijos)
    ).order_by(func.length(RateCard.destination_prefix).desc()).all()
    
    if matches:
        # El primero es el match mas largo debido al order_by length desc
        best_match = matches[0]
        
        return {
            'prefijo_id': best_match.id,
            'zona_id': best_match.id, # Mapping provisional
            'prefijo': best_match.destination_prefix,
            'zona_nombre': best_match.destination_name,
            'zona_descripcion': best_match.destination_name,
            'tarifa_segundo': float(best_match.rate_per_minute) / 60.0, # RateCard es por minuto
            'tarifa_id': best_match.id,
            'numero_valido': True
        }
    
    return {
        'prefijo_id': None,
        'zona_id': None,
        'prefijo': 'UNKNOWN',
        'zona_nombre': 'Desconocida',
        'zona_descripcion': f'Número no reconocido: {numero_marcado}',
        'tarifa_segundo': 0.0,
        'tarifa_id': None,
        'numero_valido': False
    }
