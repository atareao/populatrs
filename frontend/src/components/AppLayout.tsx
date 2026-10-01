import { useRef, useState } from "react";
import { Layout, Button, Typography, Menu } from "antd";
import {
  DashboardOutlined,
  RocketOutlined,
  TeamOutlined,
  SettingOutlined,
  LogoutOutlined,
  FileTextOutlined,
  MenuFoldOutlined,
  MenuUnfoldOutlined,
} from "@ant-design/icons";
import { Outlet, useNavigate, useLocation } from "react-router";
import { clearToken } from "../store/auth";
import { logout } from "../api/http";
import { safeRedirectTarget } from "../utils/redirect";
import { useSessionActivity } from "../hooks/useSessionActivity";

const { Content, Sider } = Layout;
const { Text } = Typography;

const menuItems = [
  { key: "/", icon: <DashboardOutlined />, label: "Dashboard" },
  { key: "/feeds", icon: <RocketOutlined />, label: "Feeds" },
  { key: "/publishers", icon: <TeamOutlined />, label: "Publishers" },
  { key: "/logs", icon: <FileTextOutlined />, label: "Logs" },
  { key: "/settings", icon: <SettingOutlined />, label: "Settings" },
];

export default function AppLayout() {
  const navigate = useNavigate();
  const location = useLocation();
  const [collapsed, setCollapsed] = useState(false);
  const [loggingOut, setLoggingOut] = useState(false);
  // Ref (not just state) so a second click within the same synchronous tick is
  // blocked before React re-renders and disables the button.
  const loggingOutRef = useRef(false);

  // Proactively end the local session on idle/absolute timeout (D6).
  useSessionActivity();

  const handleLogout = async () => {
    if (loggingOutRef.current) return;
    loggingOutRef.current = true;
    setLoggingOut(true);

    try {
      const result = await logout();
      clearToken();
      // Full navigation (not react-router) so the provider redirect works.
      window.location.href = safeRedirectTarget(result.end_session_url);
    } finally {
      // Reset so the button is not permanently disabled if navigation does
      // not occur (e.g. the redirect is blocked).
      loggingOutRef.current = false;
      setLoggingOut(false);
    }
  };

  return (
    <Layout style={{ minHeight: "100vh" }}>
      <Sider
        width={220}
        collapsedWidth={64}
        collapsible
        collapsed={collapsed}
        onCollapse={setCollapsed}
        trigger={null}
        style={{
          borderRight: "1px solid #1e1e2e",
          position: "sticky",
          top: 0,
          height: "100vh",
        }}
      >
        <div style={{ padding: "16px 20px", borderBottom: "1px solid #1e1e2e", display: "flex", alignItems: "center", justifyContent: collapsed ? "center" : "space-between" }}>
          {!collapsed && <Text className="logo-text" style={{ fontSize: 20, fontWeight: 700 }}>populatrs</Text>}
          <Button
            type="text"
            aria-label={collapsed ? "Expandir menú" : "Contraer menú"}
            icon={collapsed ? <MenuUnfoldOutlined /> : <MenuFoldOutlined />}
            onClick={() => setCollapsed(!collapsed)}
            style={{ color: "#9494a8", border: "none", padding: 4, minWidth: "auto", height: "auto" }}
          />
        </div>
        <Menu
          mode="inline"
          inlineCollapsed={collapsed}
          selectedKeys={[location.pathname]}
          items={menuItems}
          onClick={({ key }) => navigate(key)}
          style={{ marginTop: 8 }}
        />
        <div style={{ position: "absolute", bottom: 16, left: 0, right: 0, padding: "0 16px" }}>
          <Button
            type="text"
            aria-label="Cerrar sesión"
            icon={<LogoutOutlined />}
            onClick={handleLogout}
            disabled={loggingOut}
            style={{ color: "#9494a8", width: "100%", justifyContent: "flex-start" }}
          >
            {!collapsed && "Cerrar sesión"}
          </Button>
        </div>
      </Sider>
      <Layout>
        <Content style={{ padding: "24px", maxWidth: 960, margin: "0 auto", width: "100%" }}>
          <Outlet />
        </Content>
      </Layout>
    </Layout>
  );
}