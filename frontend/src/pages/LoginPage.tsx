import { useEffect, useState } from "react";
import { useNavigate, useSearchParams } from "react-router-dom";
import { Button, Card, Divider, Input, Typography, Space } from "antd";
import { GoogleOutlined, BugOutlined } from "@ant-design/icons";
import { getToken } from "../store/auth";

const { Title, Text } = Typography;

export default function LoginPage() {
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const [devEmail, setDevEmail] = useState("admin@populatrs.app");

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

  const handleDevLogin = () => {
    window.location.href = `/auth/dev-login?email=${encodeURIComponent(devEmail)}`;
  };

  return (
    <div style={{ display: "flex", justifyContent: "center", alignItems: "center", minHeight: "100vh", background: "#08080e" }}>
      <Card style={{ width: 400, textAlign: "center" }}>
        <Title level={2} className="logo-text">populatrs</Title>
        <Text type="secondary">Automatic RSS feed publisher</Text>
        <Divider />
        <Button type="primary" size="large" block icon={<GoogleOutlined />} onClick={handleLogin}>
          Iniciar sesión
        </Button>
        <Divider><Text type="secondary" style={{ fontSize: 12 }}>Desarrollo</Text></Divider>
        <Space.Compact style={{ width: "100%" }}>
          <Input value={devEmail} onChange={(e) => setDevEmail(e.target.value)} placeholder="Email para dev login" />
          <Button icon={<BugOutlined />} onClick={handleDevLogin}>Dev</Button>
        </Space.Compact>
      </Card>
    </div>
  );
}