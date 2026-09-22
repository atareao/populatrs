# Proposal

## Why

La tabla `published_posts` cumple dos funciones: (1) deduplicación — evitar que un post ya publicado se vuelva a publicar — y (2) histórico/log de publicaciones. El cleanup por retención (`cleanup_old_posts`) borra filas antiguas, lo que rompe la deduplicación: tras la limpieza, posts que siguen existiendo en el feed original se consideran "no publicados" y se vuelven a enviar a todos los publishers.

## What Changes

### Retención de deduplicación
- Eliminar la llamada a `cleanup_old_posts` en el scheduler (`main.rs`)
- Mantener `cleanup_old_publish_results` (el log por publisher no afecta a la deduplicación)
- Mantener la función `cleanup_old_posts` en `db.rs` por compatibilidad (no se usa)

### Bugfix: Conteo incorrecto en Publication History
- La query `list_feed_logs` aplica `LIMIT` sobre la JOIN con `publish_results`, no sobre los posts. Al tener N publishers por post, el límite trunca antes de agrupar, mostrando menos entradas de las que indica el total. 
- Solución: usar subquery para limitar los posts antes de la JOIN.

## Capabilities

### New Capabilities
- `deduplication/retention`: Política de retención para la tabla de deduplicación — la tabla `published_posts` nunca se limpia automáticamente (retención infinita), garantizando que un post no se republica aunque haya pasado su ventana de histórico.

### Modified Capabilities
- *(Ninguna — no hay specs existentes previamente)*

## Impact

- **backend/src/main.rs**: Eliminar llamada a `db.cleanup_old_posts()` en el scheduler (línea 373)
- **backend/src/db.rs**: No se modifica; la función `cleanup_old_posts` se conserva pero deja de invocarse
- **backend/src/db.rs (list_feed_logs)**: Corregir query para que el LIMIT se aplique sobre los posts, no sobre la JOIN con publishers
- **frontend (LogsPage.tsx línea 205)**: Actualizar el texto informativo para que indique que el cleanup automático solo aplica a los resultados de publicación (`publish_results`), no a los registros de deduplicación (`published_posts`)
- **BD**: Migración one-shot SQL para recuperar guids perdidos: re-insertar en `published_posts` cualquier post que exista en `publish_results` pero haya sido borrado de `published_posts`, evitando una tercera publicación
- **BD**: `published_posts` crece sin límite — ~26K filas/50 años en el escenario descrito (~9 MB), asumible para SQLite