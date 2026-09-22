-- Recupera en published_posts cualquier (guid, feed_id) que exista en
-- publish_results pero haya sido borrado de published_posts por el
-- antiguo cleanup automático. Ejecutar UNA SOLA VEZ antes del despliegue
-- de la versión que elimina cleanup_old_posts() del scheduler.
--
-- Ejecución:
--   sqlite3 /ruta/a/populatrs.db < backend/migrations/recover_dedup_guids.sql

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