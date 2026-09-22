# deduplication/retention Specification

## Purpose
Define la política de retención de la tabla de deduplicación `published_posts`: garantiza que un post marcado como publicado no se vuelva a publicar aunque haya transcurrido su ventana de histórico, eliminando el riesgo de re-publicaciones tras la limpieza automatizada.

## Requirements

### Requirement: Retención infinita de deduplicación
El sistema SHALL conservar todas las filas en la tabla `published_posts` de forma permanente. La limpieza automática del scheduler NO SHALL eliminar registros de `published_posts`.

#### Scenario: Post antiguo no se republica tras ciclo de limpieza
- **GIVEN** un post con `guid=X` y `feed_id=Y` que fue publicado hace 60 días
- **AND** el scheduler ejecuta `cleanup_old_publish_results` con retención de 30 días
- **WHEN** el scheduler ejecuta `run_feed_check` y el feed contiene el post con `guid=X`
- **THEN** `is_post_published("X", "Y")` devuelve `true`
- **AND** el post NO se publica de nuevo

#### Scenario: Post no publicado se publica normalmente
- **GIVEN** un post con `guid=A` y `feed_id=Y` que NO existe en `published_posts`
- **WHEN** el scheduler ejecuta `run_feed_check` y el feed contiene el post con `guid=A`
- **THEN** el post se publica en todos los publishers asignados
- **AND** se inserta una fila en `published_posts` con `guid=A` y `feed_id=Y`

#### Scenario: Múltiples ciclos no duplican publicaciones
- **GIVEN** un post con `guid=B` y `feed_id=Y` publicado en el ciclo 1
- **WHEN** el scheduler ejecuta los ciclos 2, 3, ..., N
- **THEN** `is_post_published("B", "Y")` devuelve `true` en todos los ciclos
- **AND** el post se publica exactamente una vez

### Requirement: Limpieza de resultados de publicación
El sistema SHALL mantener la limpieza automática de la tabla `publish_results` según la retención configurada (`log_retention_days`). Esta limpieza NO SHALL afectar a la tabla `published_posts`.

#### Scenario: Cleanup de publish_results no afecta a published_posts
- **GIVEN** un post con `guid=C` y `feed_id=Y` publicado hace 60 días
- **AND** `log_retention_days = 30`
- **WHEN** el scheduler ejecuta `cleanup_old_publish_results(30)`
- **THEN** las filas en `publish_results` con `guid=C` y `feed_id=Y` y `published_at` anterior a 30 días se eliminan
- **AND** la fila en `published_posts` con `guid=C` y `feed_id=Y` se conserva
- **AND** `is_post_published("C", "Y")` sigue devolviendo `true`

### Requirement: Sin límite de crecimiento en published_posts
La tabla `published_posts` SHALL crecer sin límite predefinido. No SHALL haber cleanup automático ni por tiempo ni por número de filas.

#### Scenario: Crecimiento sostenido no causa re-publicaciones
- **GIVEN** 26.050 posts publicados en `published_posts` (simulando 50 años con 521 posts/año)
- **WHEN** el scheduler ejecuta `run_feed_check` para todos los feeds
- **THEN** la consulta `SELECT 1 FROM published_posts WHERE guid = ?1 AND feed_id = ?2` se completa en tiempo constante O(log n) gracias a la PK
- **AND** ningún post publicado se republica

#### Scenario: Histórico accesible para el log
- **GIVEN** posts publicados con más de 30 días de antigüedad
- **WHEN** la UI solicita `GET /api/status/logs`
- **THEN** los posts antiguos siguen apareciendo en el histórico (ya no se filtraron por cleanup)

### Requirement: Migración one-shot de guids huérfanos
El sistema SHALL proveer un script SQL one-shot que recupere en `published_posts` cualquier `(guid, feed_id)` que exista en `publish_results` pero no en `published_posts`, evitando que esos posts se publiquen por tercera vez tras la actualización.

#### Scenario: Guids perdidos se recuperan desde publish_results
- **GIVEN** un post con `guid=X` y `feed_id=Y` que fue publicado y registrado en `publish_results`
- **AND** la fila en `published_posts` fue eliminada por el cleanup anterior
- **WHEN** se ejecuta la migración one-shot SQL
- **THEN** se inserta una fila en `published_posts` con `guid=X`, `feed_id=Y`, `title='Migrated'`, y `published_at = MIN(published_at)` de `publish_results`
- **AND** `is_post_published("X", "Y")` devuelve `true`

#### Scenario: Post existente no se duplica
- **GIVEN** un post con `guid=A` y `feed_id=Y` que existe en ambas tablas (`published_posts` y `publish_results`)
- **WHEN** se ejecuta la migración one-shot SQL
- **THEN** no se inserta ninguna fila duplicada (la cláusula `INSERT OR IGNORE` respeta la PK)

#### Scenario: Migración idempotente
- **GIVEN** la migración one-shot SQL se ejecutó una vez
- **WHEN** se ejecuta la misma migración por segunda vez
- **THEN** no se insertan nuevas filas (todos los guids ya existen en `published_posts`)

### Requirement: Listado paginado correcto en Publication History
El endpoint `GET /api/status/logs?limit=N&offset=M` SHALL devolver exactamente N entradas en `entries` cuando existan al menos N posts en `published_posts`, independientemente del número de publishers por post.

#### Scenario: Paginación con múltiples publishers
- **GIVEN** 10 posts en `published_posts`, cada uno con 7 resultados en `publish_results`
- **WHEN** se llama a `list_feed_logs(limit=5, offset=0)`
- **THEN** `entries` contiene exactamente 5 entradas
- **AND** cada entrada incluye todos sus `publisher_results`
- **AND** `total` es 10

#### Scenario: Paginación sin publishers
- **GIVEN** 10 posts en `published_posts`, ninguno con resultados en `publish_results`
- **WHEN** se llama a `list_feed_logs(limit=5, offset=0)`
- **THEN** `entries` contiene exactamente 5 entradas
