# RSS Populatrs - Publicador Automático de Feeds

Aplicación en Rust que procesa feeds RSS y videos de YouTube, publicándolos automáticamente en múltiples plataformas sociales con templating personalizable, reintentos robustos y optimizaciones HTTP.

## ✨ **Funcionalidades Principales**

### 📡 **Tipos de Feeds**

- **RSS/Atom**: Feeds tradicionales con soporte completo
- **YouTube API**: Integración directa con la API de YouTube v3
  - Por canal ID
  - Por playlist ID
  - Por username de canal
  - Extracción automática de título, descripción y metadatos

### 🎯 **8 Plataformas Soportadas**

- **Telegram**: Con soporte para topics específicos
- **X (Twitter)**: Usando API v2 con OAuth 2.0 PKCE
- **Mastodon**: Cualquier instancia
- **LinkedIn**: Publicaciones personales con OAuth 2.0
- **OpenObserve**: Logs estructurados
- **Matrix**: Salas con formato HTML
- **Bluesky**: Red social descentralizada
- **Threads**: Meta/Facebook

### 🔐 **Autenticación OAuth 2.0**

- **LinkedIn OAuth 2.0**: Setup interactivo completo con CLI
- **X (Twitter) OAuth 2.0 PKCE**: Setup seguro con PKCE flow
- **Detección automática**: Usuario vs Organización en LinkedIn
- **Refresh automático**: Los tokens se renuevan automáticamente
- **CLI interactivo**: Commands fáciles para configurar OAuth

### 🎨 **Sistema de Plantillas (Jinja2)**

- **Filtros personalizados**: `truncate`, `word_limit`, `strip_html`
- **Variables disponibles**: `{{ title }}`, `{{ description }}`, `{{ url }}`
- **Plantillas por plataforma**: Control total sobre formato y contenido

### ⚡ **Optimizaciones y Robustez**

- **Cache HTTP**: ETag y If-None-Match para reducir descargas
- **Reintentos**: Backoff exponencial configurable
- **Límite de posts**: Solo los 2 más recientes por feed
- **Detección de cambios**: Hash MD5 para evitar reprocesamiento

## 🚀 **Instalación y Uso**

### Prerrequisitos

```bash
# Rust (recomendado: rustup)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Dependencias del sistema (Ubuntu/Debian)
sudo apt update && sudo apt install -y build-essential pkg-config libssl-dev
```

### Compilación

cp config.example.json config.json

# Edita config.json con tus configuraciones

````

3. Construye y ejecuta con Docker Compose:

```bash
docker-compose up -d
````

### Compilación Manual

1. Asegúrate de tener Rust instalado:

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

2. Clona y compila:

```bash
git clone <repository-url>
cd populatrs
cargo build --release
```

3. **Setup OAuth interactivo** (recomendado):

```bash
# LinkedIn OAuth 2.0
./target/release/populatrs --linkedin-oauth --linkedin-publisher linkedin-main

# X/Twitter OAuth 2.0 PKCE
./target/release/populatrs --x-oauth --x-publisher x-main
```

4. **Ejecutar el publisher**:

```bash
./target/release/populatrs --config config.json
```

**Comandos útiles:**

```bash
# Ejecutar una vez (modo de prueba)
./target/release/populatrs --once

# Modo dry-run (solo verificar, no publicar)
./target/release/populatrs --dry-run
```

## ⚙️ Configuración

### Estructura de Configuración

```json
{
  "feeds": [
    {
      "id": "unique-id",
      "url": "https://example.com/feed.xml",
      "name": "Feed Name",
      "enabled": true,
      "publishers": ["publisher-id-1", "publisher-id-2"],
      "check_interval_minutes": 60
    }
  ],
  "publishers": {
    "publisher-id": {
      "type": "PublisherType",
      "config": {
        /* configuración específica */
      }
    }
  },
  "schedule": {
    "default_interval_minutes": 60,
    "timezone": "UTC"
  },
  "storage": {
    "data_dir": "./data",
    "published_posts_file": "published_posts.json"
  }
}
```

### Tipos de Publicadores

#### Telegram

```json
"telegram-bot": {
  "type": "Telegram",
  "config": {
    "bot_token": "123456:ABC-DEF1234ghIkl-zyx57W2v1u123ew11",
    "chat_id": "@yourchannel",
    "parse_mode": "HTML"
  }
}
```

#### X/Twitter

```json
"x-account": {
  "type": "X",
  "config": {
    "api_key": "your-api-key",
    "api_secret": "your-api-secret",
    "access_token": "your-access-token",
    "access_token_secret": "your-access-token-secret",
    "bearer_token": "your-bearer-token"
  }
}
```

#### Mastodon

```json
"mastodon-account": {
  "type": "Mastodon",
  "config": {
    "server_url": "https://mastodon.social",
    "access_token": "your-access-token"
  }
}
```

#### LinkedIn

```json
"linkedin-account": {
  "type": "LinkedIn",
  "config": {
    "client_id": "your-client-id",
    "client_secret": "your-client-secret",
    "access_token": "your-access-token",
    "user_id": "your-user-id"
  }
}
```

#### OpenObserve

```json
"openobserve-logs": {
  "type": "OpenObserve",
  "config": {
    "url": "https://your-instance.com",
    "organization": "default",
    "stream_name": "rss-feeds",
    "username": "your-username",
    "password": "your-password"
  }
}
```

#### Matrix

```json
"matrix-room": {
  "type": "Matrix",
  "config": {
    "homeserver_url": "https://matrix.org",
    "token": "your-matrix-token",
    "room_id": "!roomid:matrix.org"
  }
}
```

#### Bluesky

```json
"bluesky-account": {
  "type": "Bluesky",
  "config": {
    "handle": "your-handle.bsky.social",
    "password": "your-app-password",
    "pds_url": "https://bsky.social"
  }
}
```

#### Threads

```json
"threads-account": {
  "type": "Threads",
  "config": {
    "access_token": "your-threads-access-token",
    "user_id": "your-threads-user-id"
  }
}
```

## 🏗️ Arquitectura

### Componentes Principales

1. **Feed Manager**: Gestiona la lectura y procesamiento de feeds RSS/Atom
2. **Publisher Manager**: Maneja los diferentes tipos de publicadores
3. **Storage Manager**: Gestiona el almacenamiento de estado y configuración
4. **Scheduler**: Programa las verificaciones automáticas de feeds

### Flujo de Trabajo

1. El scheduler activa la verificación periódica
2. Feed Manager obtiene los feeds configurados
3. Se comparan con los posts ya publicados
4. Los nuevos posts se envían a Publisher Manager
5. Cada publisher publica según su plataforma
6. Se actualiza el estado de posts publicados

## 🖥️ Uso

### Opciones de Línea de Comandos

```bash
# Ejecutar continuamente (modo daemon)
populatrs --config config.json

# Ejecutar una sola vez
populatrs --config config.json --once

# Modo dry-run (no publica realmente)
populatrs --config config.json --dry-run

# Ver ayuda
populatrs --help
```

### Docker

```bash
# Ejecutar continuamente
docker-compose up -d

# Ejecutar una vez
docker run --rm -v $(pwd)/config.json:/app/config.json populatrs --once

# Logs
docker-compose logs -f
```

## 📊 Monitoreo y Logs

La aplicación genera logs detallados de todas las operaciones:

- `INFO`: Operaciones normales y estadísticas
- `WARN`: Situaciones que requieren atención
- `ERROR`: Errores que necesitan intervención

### Variables de Entorno

- `RUST_LOG`: Nivel de logging (`error`, `warn`, `info`, `debug`, `trace`)

## � Setup OAuth 2.0 Interactivo

### LinkedIn OAuth 2.0

Para configurar LinkedIn OAuth de forma **sencilla e intuitiva**:

```bash
# Setup OAuth para LinkedIn
./target/release/populatrs --linkedin-oauth --linkedin-publisher linkedin-main
```

**Proceso interactivo:**

1. Se abre automáticamente la URL de autorización en el navegador
2. Inicias sesión en LinkedIn y autorizas la app
3. Copias el código de la URL de callback
4. Lo pegas en la terminal
5. ¡Listo! Los tokens se guardan automáticamente

### X (Twitter) OAuth 2.0 PKCE

Para configurar X OAuth con **flujo PKCE seguro**:

```bash
# Setup OAuth para X/Twitter
./target/release/populatrs --x-oauth --x-publisher x-main
```

**Proceso interactivo:**

1. Se genera automáticamente una URL con PKCE challenge
2. Se abre la URL de autorización de X
3. Autorizas la aplicación en X
4. Copias el código de autorización
5. Los tokens OAuth se guardan con refresh automático

### Configuración Previa Requerida

Antes del setup OAuth, configura tu `config.json`:

```json
{
  "publishers": {
    "linkedin-main": {
      "type": "LinkedIn",
      "config": {
        "client_id": "TU_CLIENT_ID",
        "client_secret": "TU_CLIENT_SECRET",
        "redirect_uri": "http://localhost:8080/callback",
        "access_token": null,
        "refresh_token": null,
        "user_id": null,
        "organization_id": null,
        "template": "{{ title }}\\n\\n{{ description | truncate(700) }}\\n\\nLeer más: {{ url }}"
      }
    },
    "x-main": {
      "type": "X",
      "config": {
        "client_id": "TU_X_CLIENT_ID",
        "client_secret": "TU_X_CLIENT_SECRET",
        "redirect_uri": "http://localhost:8080/callback",
        "access_token": null,
        "refresh_token": null,
        "template": "🚀 {{ title | truncate(210) }}\\n\\n🔗 {{ url }} #RSS"
      }
    }
  }
}
```

**📋 Documentación detallada:**

- [LinkedIn OAuth Setup](docs/linkedin-oauth-setup.md)
- [X OAuth Setup](docs/x-oauth-setup.md)

## �🔧 Configuración de Plataformas

### Telegram

1. Crea un bot con [@BotFather](https://t.me/BotFather)
2. Obtén el token del bot
3. Añade el bot al canal/grupo y obtén el chat_id

### X/Twitter

1. Registra una aplicación en [Twitter Developer](https://developer.twitter.com)
2. Obtén las credenciales API
3. Configura los permisos necesarios

### Mastodon

1. Ve a Configuración → Desarrollo en tu instancia
2. Crea una nueva aplicación
3. Obtén el token de acceso

### LinkedIn

1. Crea una aplicación en [LinkedIn Developers](https://www.linkedin.com/developers/)
2. Configura los permisos de publicación
3. Obtén las credenciales OAuth

### OpenObserve

1. Configura tu instancia de OpenObserve
2. Crea un stream para los logs
3. Obtén las credenciales de acceso

### Matrix

1. Obtén un token de acceso para tu cuenta
2. Encuentra el ID de la sala donde publicar
3. Asegúrate de que el bot tenga permisos de escritura

### Bluesky

1. Crea una cuenta en Bluesky (bsky.app)
2. Genera una contraseña de aplicación en Settings > App Passwords
3. Usa tu handle completo (ej: usuario.bsky.social)
4. Para servidores personalizados, configura el campo `pds_url`

### Threads

1. Registra una aplicación en Meta for Developers
2. Configura los permisos necesarios para Threads API
3. Obtén el access token y user ID
4. Asegúrate de cumplir con las políticas de contenido de Meta

## ⚡ Optimización HTTP y Reducción de Descargas

Populatrs implementa múltiples estrategias para minimizar el ancho de banda y mejorar el rendimiento:

### ETag y If-None-Match

- **Detección automática**: Captura ETags de respuestas HTTP
- **Requests condicionales**: Usa `If-None-Match` en siguientes requests
- **304 Not Modified**: Evita descargar contenido sin cambios
- **Persistencia**: ETags se guardan automáticamente en `feed_cache.json`

### Last-Modified y If-Modified-Since

- **Headers temporales**: Usa `If-Modified-Since` cuando está disponible
- **Cache coordinado**: Combina ETag y Last-Modified para máxima eficiencia

### Hash MD5 del Contenido

- **Detección de cambios**: Verifica si el contenido realmente cambió
- **Redundancia**: Funciona incluso si el servidor no soporta ETags
- **Hash persistente**: Se almacena para comparaciones futuras

### Beneficios

✅ **Reducción drástica** del ancho de banda utilizado  
✅ **Menor carga** en los servidores de feeds RSS  
✅ **Respuesta más rápida** en feeds sin cambios  
✅ **Detección inteligente** de contenido duplicado

Los logs mostrarán cuando se evita una descarga:

```
[INFO] Feed Example Blog not modified (304), skipping download
[INFO] Feed Tech News content unchanged (same hash), skipping parse
```

## 🚨 Solución de Problemas

### Problemas Comunes

1. **Error de parsing de feed**: Verifica que la URL del feed sea válida
2. **Error de autenticación**: Revisa las credenciales de los publicadores
3. **Rate limiting**: Ajusta los intervalos de verificación
4. **Permisos de archivo**: Verifica los permisos del directorio de datos

### Debug

```bash
# Ejecutar con logs detallados
RUST_LOG=debug populatrs --config config.json

# Probar configuración
populatrs --config config.json --dry-run --once
```

## 🤝 Contribuir

1. Fork el proyecto
2. Crea una rama para tu feature (`git checkout -b feature/AmazingFeature`)
3. Commit tus cambios (`git commit -m 'Add some AmazingFeature'`)
4. Push a la rama (`git push origin feature/AmazingFeature`)
5. Abre un Pull Request

## 📝 Licencia

Este proyecto está bajo la licencia MIT. Ver `LICENSE` para más detalles.

## 🆘 Soporte

- Reporta bugs en [Issues](../../issues)
- Para preguntas, usa [Discussions](../../discussions)

## ✅ TODO

- [ ] Soporte para más plataformas (Discord, Slack, etc.)
- [ ] API REST para gestión remota
- [ ] Templates personalizables para posts
- [ ] Filtros de contenido
- [ ] Métricas y analytics
- [ ] Interfaz web de administración
