import { Layout, Button, Typography, Space, Menu } from "antd";
import {
  DashboardOutlined,
  RocketOutlined,
  TeamOutlined,
  ScheduleOutlined,
  SettingOutlined,
  LogoutOutlined,
  FileTextOutlined,
} from "@ant-design/icons";
import { Outlet, useNavigate, useLocation } from "react-router-dom";
import { clearToken } from "../store/auth";

const { Content, Sider } = Layout;
const { Text } = Typography;

const menuItems = [
  { key: "/", icon: <DashboardOutlined />, label: "Dashboard" },
  { key: "/feeds", icon: <RocketOutlined />, label: "Feeds" },
  { key: "/publishers", icon: <TeamOutlined />, label: "Publishers" },
  { key: "/schedule", icon: <ScheduleOutlined />, label: "Schedule" },
  { key: "/settings", icon: <SettingOutlined />, label: "Settings" },
  { key: "/logs", icon: <FileTextOutlined />, label: "Logs" },
];

export default function AppLayout() {
  const navigate = useNavigate();
  const location = useLocation();

  const handleLogout = () => {
    clearToken();
    navigate("/login", { replace: true });
  };

  return (
    <Layout style={{ minHeight: "100vh" }}>
      <Sider
        width={220}
        style={{
          borderRight: "1px solid #1e1e2e",
          position: "sticky",
          top: 0,
          height: "100vh",
        }}
      >
        <div style={{ padding: "16px 20px", borderBottom: "1px solid #1e1e2e" }}>
          <Text className="logo-text" style={{ fontSize: 20, fontWeight: 700 }}>
            populatrs
          </Text>
        </div>
        <Menu
          mode="inline"
          selectedKeys={[location.pathname]}
          items={menuItems}
          onClick={({ key }) => navigate(key)}
          style={{ marginTop: 8 }}
        />
        <div style={{ position: "absolute", bottom: 16, left: 0, right: 0, padding: "0 16px" }}>
          <Button
            type="text"
            icon={<LogoutOutlined />}
            onClick={handleLogout}
            style={{ color: "#9494a8", width: "100%", justifyContent: "flex-start" }}
          >
            Cerrar sesión
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