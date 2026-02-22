import logging
import requests
import uuid
import secrets
import urllib.parse


logger = logging.getLogger(__name__)
TIMEOUT = (5, 60)


class LinkedInException(Exception):
    pass


class LinkedIn:
    def __init__(self, organization=None, token=None):
        self._base_url = "https://api.linkedin.com"
        self._organization = organization
        self._headers = {
            "Authorization": f"Bearer {token}",
            "X-Restli-Protocol-Version": "2.0.0",
            "Content-Type": "application/json",
            "Access-Control-Allow-Origin": "*"
        }

    def init_client(self, client_id: str, client_secret: str):
        self._client_id = client_id
        self._client_secret = client_secret

    def get_auth_code(self):
        url = "https://www.linkedin.com/oauth/v2/authorization"
        params = {
            "response_type": "code",
            "client_id": self._client_id,
            "redirect_uri": "http://localhost:3000/callback",
            "state": secrets.token_urlsafe(16),
            #"scope": "openid email profile w_member_social"
            "scope": "w_organization_social r_organization_social"
        }
        auth_url = "https://www.linkedin.com/oauth/v2/authorization?" + urllib.parse.urlencode(params)
        print(auth_url)
        response = requests.get(url, params=params, timeout=TIMEOUT)
        if not response.ok:
            raise LinkedInException(f"Error: {response.text}")
        return response.text

    def get_access_token(self, code):
        print("==========================")
        print(code)
        print("==========================")
        payload = {
            "grant_type": "authorization_code",
            "code": code,
            "redirect_uri": "http://localhost:3000/callback",
            "client_id": self._client_id,
            "client_secret": self._client_secret
        }
        headers = {
            "Content-Type": "application/x-www-form-urlencoded",
        }
        try:
            response = requests.post(
                "https://www.linkedin.com/oauth/v2/accessToken",
                headers=headers,
                data=payload,
            )
            response.raise_for_status() # Lanza una excepción si la solicitud no fue exitosa

            token_data = response.json()
            access_token = token_data.get("access_token")
            expires_in = token_data.get("expires_in") # Tiempo de vida en segundos
            refresh_token = token_data.get("refresh_token") # Si solicitaste el scope r_liteprofile

            if access_token:
                print("Access Token obtenido exitosamente:")
                print(f"Access Token: {access_token}")
                print(f"Expira en: {expires_in} segundos")
                if refresh_token:
                    print(f"Refresh Token: {refresh_token}")
                    # ¡IMPORTANTE! Almacena el refresh_token de forma segura para renovaciones futuras.
                return access_token, refresh_token, expires_in
            else:
                print("Error: No se recibió un access_token en la respuesta.")
                print(f"Respuesta completa: {token_data}")
                return None, None, None

        except requests.exceptions.HTTPError as err:
            print(f"Error HTTP al obtener el token: {err}")
            print(f"Respuesta de LinkedIn: {response.text}")
            return None, None, None
        except Exception as e:
            print(f"Ocurrió un error inesperado al obtener el token: {e}")
            return None, None, None

    def send_message(self, message: str):
        logger.info("send_message: %s", message)
        payload = {
            "author": f"urn:li:organization:{self._organization}",
            "lifecycleState": "PUBLISHED",
            "specificContent": {
                "com.linkedin.ugc.ShareContent": {
                    "shareCommentary": {
                        "text": commentary
                    },
                    "shareMediaCategory": "ARTICLE",
                    "media": [{
                        "status": "READY",
                        "description": {
                            "text": post.description.as_deref().unwrap_or("")
                        },
                        "originalUrl": post.link,
                        "title": {
                            "text": post.title
                        }
                    }]
                }
            },
            "visibility": {
                "com.linkedin.ugc.MemberNetworkVisibility": "PUBLIC"
            }
        }
        print("payload: %s", payload)
        url = f"{self._base_url}/v2/ugcPosts"
        response = requests.post(url, json=payload, headers=self._headers,
                                 timeout=TIMEOUT)
        if not response.ok:
            raise LinkedInException(f"Error: {response.text}")
        return response

if __name__ == "__main__":
    import os
    from dotenv import load_dotenv
    load_dotenv()
    organization = os.getenv("LINKEDIN_ORGANIZATION", "")
    access_token = os.getenv("LINKEDIN_ACCESS_TOKEN", "")
    client_id = os.getenv("LINKEDIN_CLIENT_ID", "")
    client_secret = os.getenv("LINKEDIN_CLIENT_SECRET", "")
    print(organization, access_token)
    client = LinkedIn(organization, access_token)
    client.init_client(client_id, client_secret)
    #client.get_auth_code()
    #exit(0)
    #token = os.getenv("CUSTOM_TOKEN", "")
    #print(token)
    #client.get_access_token(token)
    client.send_message("test")
