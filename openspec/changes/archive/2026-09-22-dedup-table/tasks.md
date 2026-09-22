# Tasks

## 1. Eliminar cleanup automático de published_posts

- [ ] 1.1 Eliminar la llamada a `db.cleanup_old_posts()` en `backend/src/main.rs` (línea 373) y verificar que `cargo check` pasa sin errores
- [ ] 1.2 Ejecutar `cargo test` para confirmar que todos los tests existentes siguen pasando (GREEN)

## 2. Migración one-shot para guids huérfanos

- [ ] 2.1 Crear archivo `backend/migrations/recover_dedup_guids.sql` con el SQL de recuperación y verificar que se ejecuta correctamente contra la BD (simulacro con `sqlite3 :memory:`)
- [ ] 2.2 Documentar la migración en el README o CHANGELOG como paso pre-despliegue

## 3. Frontend

- [ ] 3.1 Actualizar texto en `frontend/src/pages/LogsPage.tsx` línea 205 para indicar que el cleanup automático solo aplica a publish_results, no a deduplicación

## 4. Bugfix paginación en list_feed_logs

- [ ] 4.1 Modificar la query SQL en `backend/src/db.rs:list_feed_logs` para usar subquery que limite los posts antes de la JOIN con publish_results
- [ ] 4.2 Verificar que `cargo test` pasa y que las entradas devueltas coinciden con el límite solicitado

## 5. Verificación

- [ ] 5.1 Ejecutar `cargo clippy -- -D warnings` y confirmar 0 warnings
- [ ] 5.2 Ejecutar `cargo fmt --check` y confirmar formato correcto
- [ ] 5.3 Verificar que `cargo build` compila sin errores
- [ ] 5.4 Confirmar que la función `cleanup_old_posts` en `db.rs` sigue existiendo (no se eliminó, solo dejó de invocarse)