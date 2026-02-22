# Configuración OAuth 2.0 para X (Twitter)

Este documento explica cómo configurar la autenticación OAuth 2.0 con PKCE para el publisher de X (Twitter).

## Prerrequisitos

1. Una cuenta de desarrollador de X (Twitter)
2. Una aplicación registrada en el [Portal de Desarrolladores de X](https://developer.twitter.com/en/portal/dashboard)
3. Permisos de escritura habilitados para tu aplicación
4. Configurada la URL de callback: `http://localhost:8080/callback`

## Configuración de la Aplicación en X

1. **Accede al Portal de Desarrolladores**: https://developer.twitter.com/en/portal/dashboard
2. **Selecciona tu aplicación** o crea una nueva
3. **Ve a "Settings" > "User authentication settings"**
4. **Configura OAuth 2.0 settings**:
   - **Type of App**: Web App, Automated App or Bot
   - **App permissions**: Read and write
   - **Callback URI / Redirect URL**: `http://localhost:8080/callback`
   - **Website URL**: Tu sitio web o `https://example.com`

5. **Guarda las credenciales**:
   - **Client ID**: Lo necesitarás para la configuración
   - **Client Secret**: Lo necesitarás para la configuración (¡mantén esto seguro!)

## Configuración en Populatrs

### Paso 1: Configura el publisher en tu `config.json`

```json
{
  "publishers": {
    "x-main": {
      "type": "X",
      "config": {
        "client_id": "TU_CLIENT_ID_DE_X",
        "client_secret": "TU_CLIENT_SECRET_DE_X",
        "redirect_uri": "http://localhost:8080/callback",
        "access_token": null,
        "refresh_token": null,
        "template": "{{ title | truncate(240) }}\\n\\n{{ url }}"
      }
    }
  }
}
```

### Paso 2: Ejecuta el setup interactivo de OAuth

```bash
# Reemplaza "x-main" con el ID de tu publisher
./target/release/populatrs --x-oauth --x-publisher x-main
```

### Paso 3: Sigue el proceso interactivo

1. **Se abrirá automáticamente** una URL de autorización en tu navegador
2. **Inicia sesión en X** y autoriza la aplicación
3. **Copia el código de autorización** de la URL de callback
4. **Pégalo en la terminal** cuando se te solicite
5. **¡Listo!** Los tokens se guardarán automáticamente en tu configuración

## Proceso OAuth 2.0 PKCE

El sistema utiliza OAuth 2.0 con PKCE (Proof Key for Code Exchange), que es más seguro que el flujo básico:

1. **Generación de PKCE**: Se crea un `code_verifier` aleatorio y su `code_challenge`
2. **URL de autorización**: Se incluyen los parámetros PKCE en la URL de autorización
3. **Intercambio de código**: Se intercambia el código de autorización por tokens usando el `code_verifier`
4. **Refresh automático**: Los tokens se refrescan automáticamente cuando expiran

## URLs de OAuth 2.0 de X

- **Autorización**: `https://twitter.com/i/oauth2/authorize`
- **Token**: `https://api.twitter.com/2/oauth2/token`

## Scopes y Permisos

- **tweet.read**: Leer tweets
- **tweet.write**: Crear tweets
- **users.read**: Leer información básica del usuario
- **offline.access**: Acceso para refresh tokens

## Solución de Problemas

### Error: "Invalid redirect_uri"

- Verifica que la URL de callback en tu aplicación de X sea exactamente: `http://localhost:8080/callback`
- Asegúrate de que no haya espacios adicionales o caracteres especiales

### Error: "Invalid client credentials"

- Verifica que el `client_id` y `client_secret` sean correctos
- Asegúrate de que no haya espacios adicionales al copiar las credenciales

### Error: "Insufficient permissions"

- Ve a tu aplicación en el Portal de Desarrolladores
- Asegúrate de que tenga permisos de "Read and write"
- Regenera las credenciales si es necesario

### Tokens expirados

- Los tokens se refrescan automáticamente
- Si hay problemas, ejecuta el comando de setup nuevamente

## Notas de Seguridad

- **Nunca compartas** tu `client_secret`
- **No commites** archivos de configuración con tokens reales en repositorios públicos
- **Usa variables de entorno** en producción si es posible
- **Regenera credenciales** si sospechas que han sido comprometidas

## Plantillas Recomendadas

### Tweet básico:

```
{{ title | truncate(240) }}

{{ url }}
```

### Tweet con descripción:

```
{{ title | truncate(150) }}

{{ description | truncate(80) }}

{{ url }}
```

### Tweet con hashtags:

```
{{ title | truncate(200) }} #RSS #AutoPost

{{ url }}
```

## Ejemplo Completo

Archivo `config.json` completo con X configurado:

```json
{
  "feeds": [
    {
      "id": "mi-blog",
      "type": "Rss",
      "config": {
        "url": "https://miblog.com/feed.xml"
      },
      "name": "Mi Blog Personal",
      "enabled": true,
      "publishers": ["x-main"],
      "check_interval_minutes": 60
    }
  ],
  "publishers": {
    "x-main": {
      "type": "X",
      "config": {
        "client_id": "VE9ETElEV1MyRkFLRUNMSUVOVElEMTIz",
        "client_secret": "dGhpc2lzYWZha2VzZWNyZXRkb250dXNldGhpc2lucmVhbGFwcGxpY2F0aW9ucw",
        "redirect_uri": "http://localhost:8080/callback",
        "access_token": null,
        "refresh_token": null,
        "template": "🚀 {{ title | truncate(210) }}\\n\\n🔗 {{ url }} #Blog #RSS"
      }
    }
  }
}
```

## Comandos útiles

```bash
# Setup OAuth para X
./target/release/populatrs --x-oauth --x-publisher x-main

# Ejecutar una vez (modo de prueba)
./target/release/populatrs --once

# Modo dry-run (solo verificar, no publicar)
./target/release/populatrs --dry-run
```
