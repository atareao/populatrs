# Design

## Context

Ver proposal.md — la tabla `published_posts` hace doble función (deduplicación + histórico) y su cleanup por retención causa re-publicaciones. El fix es mínimo: eliminar la llamada a `cleanup_old_posts` del scheduler. Además:

- La UI de LogsPage muestra un texto sobre cleanup que debe corregirse
- Hace falta una migración one-shot para recuperar guids perdidos en `published_posts` desde `publish_results`
- La query `list_feed_logs` aplica `LIMIT` sobre la JOIN en lugar de sobre los posts, causando que se devuelvan menos entradas de las solicitadas cuando hay múltiples publishers por post

La tabla tiene PK `(guid, feed_id)` que ya funciona como índice de cobertura para `is_post_published`. La FK de `publish_results` con `ON DELETE CASCADE` queda huérfana pero inofensiva: nunca se dispara porque no se borran padres.

## Goals / Non-Goals

**Goals:**
- Eliminar la fuente de re-publicaciones: que `cleanup_old_posts` no se ejecute nunca automáticamente
- Mantener la limpieza de `publish_results` para controlar el tamaño del log histórico
- Actualizar el texto de LogsPage para reflejar que el retention solo aplica a publish_results
- Proveer migración one-shot SQL para recuperar guids perdidos antes del despliegue
- Corregir la paginación de `list_feed_logs` para que el LIMIT se aplique sobre posts, no sobre la JOIN

**Non-Goals:**
- No se añaden nuevas tablas ni columnas
- No se cambia el schema de BD
- No se modifica la lógica de `is_post_published` ni `mark_post_published`
- No se añaden nuevos endpoints API
- No se cambia el comportamiento del control de retention en la UI

## Decisions

1. **Eliminar solo la llamada, no la función**: `cleanup_old_posts` se conserva en `db.rs` por si alguien la necesita para una limpieza manual/one-off. No se invoca desde ningún sitio del flujo normal.

2. **PublishedPostsStorage.cleanup_old_posts**: Es un método de la estructura en memoria (`models.rs:269`) usada en modo legacy/file-based. Se deja intacto — no afecta al flujo SQLite que es el que usa el scheduler.

3. **No tocar el modelo de datos**: La PK compuesta `(guid, feed_id)` en `published_posts` ya es óptima para el lookup de deduplicación. No se necesita migración de schema.

4. **Migración one-shot vía script SQL**: No se añade lógica en Rust. Se provee un archivo `migrations/recover_dedup_guids.sql` que el usuario ejecuta manualmente antes del despliegue. Es más seguro que código automático que podría ejecutarse en el momento equivocado.

5. **Frontend**: Solo texto informativo en LogsPage.tsx línea 205. El control de retention y el endpoint `/api/logs/retention` siguen funcionando igual, pero el texto aclara que solo afecta a publish_results.

6. **Bugfix list_feed_logs**: La query actual:
   ```sql
   SELECT pp.guid, ..., pr.publisher_id, ...
   FROM published_posts pp
   LEFT JOIN publish_results pr ON ...
   ORDER BY pp.published_at DESC
   LIMIT ?1 OFFSET ?2
   ```
   Con N publishers por post, el `LIMIT` trunca sobre las filas joinadas. Solución: subquery que limite los posts antes de la JOIN:
   ```sql
   SELECT pp.guid, ..., pr.publisher_id, ...
   FROM (SELECT * FROM published_posts ORDER BY published_at DESC LIMIT ?1 OFFSET ?2) pp
   LEFT JOIN publish_results pr ON pp.guid = pr.guid AND pp.feed_id = pr.feed_id
   ORDER BY pp.published_at DESC
   ```

## Migración SQL

```sql
INSERT OR IGNORE INTO published_posts (guid, feed_id, title, url, description, published_at)
SELECT DISTINCT pr.guid, pr.feed_id,
       COALESCE(pp.title, 'Migrated'),
       COALESCE(pp.url, ''),
       pp.description,
       MIN(pr.published_at)
FROM publish_results pr
LEFT JOIN published_posts pp ON pp.guid = pr.guid AND pp.feed_id = pr.feed_id
WHERE pp.guid IS NULL
GROUP BY pr.guid, pr.feed_id;
```

## Riesgos

- [Crecimiento ilimitado] → ~26K filas en 50 años para el escenario típico (~9 MB). SQLite manejado.
- [publish_results sin FK efectiva] → Si se borra un padre manualmente (operación que no ocurre en flujo normal), los hijos quedan huérfanos. Acceptable: son datos de log, no críticos.
- [Rollback] → Si se quiere revertir, basta con volver a añadir la llamada a `cleanup_old_posts`.
- [Migración SQL ejecutada en el momento equivocado] → Se provee como script separado, no como parte del código de la app. El usuario decide cuándo ejecutarla.