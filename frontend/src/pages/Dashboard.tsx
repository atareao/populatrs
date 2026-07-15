import { useEffect, useState } from "react";
import { Card, Col, Row, Statistic, Typography, Spin, Descriptions, Tag } from "antd";
import {
  RocketOutlined, TeamOutlined, CheckCircleOutlined, CloseCircleOutlined,
  ClockCircleOutlined, GlobalOutlined, DatabaseOutlined, FileTextOutlined,
  InboxOutlined,
} from "@ant-design/icons";
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

  if (loading) return <div style={{ textAlign: "center", padding: 40 }}><Spin size="large" data-testid="spinner" /></div>;
  if (!status) return <div style={{ textAlign: "center", padding: 40 }}><Typography.Text type="danger">Failed to load dashboard data</Typography.Text></div>;

  return (
    <div className="fade-in-up">
      <Title level={3}>Dashboard</Title>

      {/* ── Feed stats ── */}
      <Row gutter={[16, 16]}>
        <Col xs={24} sm={12} lg={6}>
          <Card>
            <Statistic
              title="Feeds Activos"
              value={status.feeds.enabled}
              suffix={`/ ${status.feeds.total}`}
              prefix={<RocketOutlined style={{ color: "#22c55e" }} />}
            />
          </Card>
        </Col>
        <Col xs={24} sm={12} lg={6}>
          <Card>
            <Statistic
              title="Feeds Inactivos"
              value={status.feeds.disabled}
              prefix={<CloseCircleOutlined style={{ color: "#ef4444" }} />}
            />
          </Card>
        </Col>
        <Col xs={24} sm={12} lg={6}>
          <Card>
            <Statistic
              title="Publishers"
              value={status.publishers.total}
              prefix={<TeamOutlined style={{ color: "#60a5fa" }} />}
            />
          </Card>
        </Col>
        <Col xs={24} sm={12} lg={6}>
          <Card>
            <Statistic
              title="Posts Publicados"
              value={status.published_posts}
              prefix={<InboxOutlined style={{ color: "#a78bfa" }} />}
            />
          </Card>
        </Col>
      </Row>

      {/* ── Schedule ── */}
      <Row gutter={[16, 16]} style={{ marginTop: 16 }}>
        <Col xs={24} md={12}>
          <Card title={<><ClockCircleOutlined /> Schedule</>}>
            <Descriptions column={1} size="small">
              <Descriptions.Item label="Intervalo">
                {status.schedule.interval_minutes} min
              </Descriptions.Item>
              <Descriptions.Item label="Zona Horaria">
                <Tag icon={<GlobalOutlined />}>{status.schedule.timezone}</Tag>
              </Descriptions.Item>
            </Descriptions>
          </Card>
        </Col>
        <Col xs={24} md={12}>
          <Card title={<><DatabaseOutlined /> Storage</>}>
            <Descriptions column={1} size="small">
              <Descriptions.Item label="Data Directory">
                <code>{status.storage.data_dir}</code>
              </Descriptions.Item>
              <Descriptions.Item label="Published Posts File">
                <code>{status.storage.published_posts_file}</code>
              </Descriptions.Item>
            </Descriptions>
          </Card>
        </Col>
      </Row>
    </div>
  );
}
