import { useEffect, useState } from "react";
import { Card, Col, Row, Statistic, Typography, Spin, Descriptions, Tag } from "antd";
import {
  RocketOutlined, TeamOutlined, CloseCircleOutlined,
  ClockCircleOutlined, GlobalOutlined, DatabaseOutlined,
  InboxOutlined, FieldTimeOutlined, CalendarOutlined,
} from "@ant-design/icons";
import { fetchStatus, type DashboardStatus } from "../api/http";
import dayjs from "dayjs";
import relativeTime from "dayjs/plugin/relativeTime";

dayjs.extend(relativeTime);

const { Title } = Typography;

function formatTime(iso: string | null): string {
  if (!iso) return "—";
  const d = dayjs(iso);
  return `${d.fromNow()} (${d.format("HH:mm:ss")})`;
}

export default function Dashboard() {
  const [status, setStatus] = useState<DashboardStatus | null>(null);
  const [loading, setLoading] = useState(true);

  useEffect(() => {
    const load = () =>
      fetchStatus()
        .then(setStatus)
        .catch(console.error)
        .finally(() => setLoading(false));
    load();
    const interval = setInterval(load, 30_000);
    return () => clearInterval(interval);
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

      {/* ── Scheduler timing ── */}
      <Row gutter={[16, 16]} style={{ marginTop: 16 }}>
        <Col xs={24} sm={12}>
          <Card title={<><FieldTimeOutlined /> Última ejecución</>}>
            <Title level={4} style={{ margin: 0, color: "#22c55e" }}>
              {formatTime(status.last_run_at)}
            </Title>
          </Card>
        </Col>
        <Col xs={24} sm={12}>
          <Card title={<><CalendarOutlined /> Próxima ejecución</>}>
            <Title level={4} style={{ margin: 0, color: "#60a5fa" }}>
              {formatTime(status.next_run_at)}
            </Title>
          </Card>
        </Col>
      </Row>

      {/* ── Schedule & Storage ── */}
      <Row gutter={[16, 16]} style={{ marginTop: 16 }}>
        <Col xs={24} md={12}>
          <Card title={<><ClockCircleOutlined /> Schedule</>}>
            <Descriptions column={1} size="small">
              <Descriptions.Item label="Cron">
                {status.schedule.cron_expression}
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
            </Descriptions>
          </Card>
        </Col>
      </Row>
    </div>
  );
}