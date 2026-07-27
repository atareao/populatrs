import { useEffect } from "react";
import { useNavigate, useSearchParams } from "react-router";
import { Button, Card, Typography, Divider, Image } from "antd";
import { LoginOutlined } from "@ant-design/icons";
import { getToken } from "../store/auth";

const { Title, Text } = Typography;

export default function LoginPage() {
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();

  useEffect(() => {
    const token = searchParams.get("token");
    if (token) {
      localStorage.setItem("populatrs_token", token);
      sessionStorage.setItem("populatrs_token", token);
      navigate("/", { replace: true });
    }
    const existingToken = getToken();
    if (existingToken) {
      navigate("/", { replace: true });
    }
  }, [searchParams, navigate]);

  const handleLogin = () => {
    window.location.href = "/auth/login";
  };

  return (
    <div style={{ display: "flex", justifyContent: "center", alignItems: "center", minHeight: "100vh", background: "#08080e" }}>
      <Card style={{ width: 400, textAlign: "center" }}>
        <Image
          src="/icono-192x192.png"
          width={96}
          preview={false}
          style={{ display: "block", margin: "0 auto 12px" }}
        />
        <Title level={2} className="logo-text">populatrs</Title>
        <Text type="secondary">Automatic RSS feed publisher</Text>
        <Divider />
        <Button type="primary" size="large" block icon={<LoginOutlined />} onClick={handleLogin}>
          Iniciar con OIDC
        </Button>
      </Card>
    </div>
  );
}