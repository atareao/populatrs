import { useEffect, useState } from "react";
import { Card, Col, Row, Statistic, Typography, Spin } from "antd";
import { RocketOutlined, TeamOutlined, CheckCircleOutlined, CloseCircleOutlined } from "@ant-design/icons";
import { fetchStatus, type DashboardStatus } from "../api/http";

const { Title } = Typography;

export default function Dashboard() {
  const [status, setStatus] = useState<DashboardStatus | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    fetchStatus()
      .then(setStatus)
      .catch(console.error)
      .finally(() => setLoading(false));
  }, []);

  if (loading) return <div style={{ textAlign: "center", padding: 40 }}><Spin size="large" /></div>;

  return (
    <div>
      <Title level={3}>Dashboard</Title>
      <Row gutter={[16, 16]}>
        <Col span={8}>
          <Card>
            <Statistic
              title="Feeds Activos"
              value={status?.feeds.enabled ?? 0}
              suffix={`/ ${status?.feeds.total ?? 0}`}
              prefix={<RocketOutlined style={{ color: "#22c55e" }} />}
            />
          </Card>
        </Col>
        <Col span={8}>
          <Card>
            <Statistic
              title="Feeds Inactivos"
              value={status?.feeds.disabled ?? 0}
              prefix={<CloseCircleOutlined style={{ color: "#ef4444" }} />}
            />
          </Card>
        </Col>
        <Col span={8}>
          <Card>
            <Statistic
              title="Publishers"
              value={status?.publishers.total ?? 0}
              prefix={<TeamOutlined style={{ color: "#60a5fa" }} />}
            />
          </Card>
        </Col>
      </Row>

      <Row gutter={[16, 16]} style={{ marginTop: 16 }}>
        <Col span={12}>
          <Card title="Schedule">
            <Statistic
              title="Intervalo"
              value={status?.schedule.interval_minutes ?? 60}
              suffix="min"
            />
            <div style={{ marginTop: 12 }}>
              <Statistic
                title="Zona Horaria"
                value={status?.schedule.timezone ?? "UTC"}
              />
            </div>
          </Card>
        </Col>
        <Col span={12}>
          <Card title="Storage">
            <Statistic
              title="Data Directory"
              value={status?.storage.data_dir ?? "./data"}
            />
            <div style={{ marginTop: 12 }}>
              <Statistic
                title="Published Posts File"
                value={status?.storage.published_posts_file ?? "published_posts.json"}
              />
            </div>
          </Card>
        </Col>
      </Row>
    </div>
  );
}