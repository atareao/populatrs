import { useEffect, useState } from "react";
import { Card, Form, Input, Select, Button, Typography, message, Spin, Alert, Tag } from "antd";
import { SaveOutlined, InfoCircleOutlined, YoutubeOutlined, ClockCircleOutlined, GlobalOutlined, LinkOutlined } from "@ant-design/icons";
import { fetchYoutubeConfig, updateYoutubeConfig, fetchSchedule, updateSchedule, type ScheduleConfig } from "../api/http";

const { Title } = Typography;

const CRON_PRESETS = [
  { label: "Every 5 minutes", value: "*/5 * * * *" },
  { label: "Every 15 minutes", value: "*/15 * * * *" },
  { label: "Every 30 minutes", value: "*/30 * * * *" },
  { label: "Every hour", value: "0 * * * *" },
  { label: "Every 6 hours", value: "0 */6 * * *" },
  { label: "Daily at 06:00", value: "0 6 * * *" },
  { label: "Daily at 06:00, 10:00, 12:00", value: "0 6,10,12 * * *" },
  { label: "Daily at 22:00", value: "0 22 * * *" },
];

export default function Settings() {
  const [loading, setLoading] = useState(true);
  const [ytSaving, setYtSaving] = useState(false);
  const [schedSaving, setSchedSaving] = useState(false);
  const [currentCron, setCurrentCron] = useState<string>("0 * * * *");
  const [currentTimezone, setCurrentTimezone] = useState<string>("UTC");
  const [ytForm] = Form.useForm();
  const [schedForm] = Form.useForm();

  useEffect(() => {
    Promise.all([
      fetchYoutubeConfig().then((d) => ytForm.setFieldsValue(d)),
      fetchSchedule().then((d) => {
        schedForm.setFieldsValue(d);
        setCurrentCron(d.cron_expression);
        setCurrentTimezone(d.timezone);
      }),
    ])
      .catch(() => message.error("Failed to load config"))
      .finally(() => setLoading(false));
  }, [ytForm, schedForm]);

  const handleYtSubmit = async (values: { api_key: string }) => {
    setYtSaving(true);
    try {
      await updateYoutubeConfig(values);
      message.success("YouTube config saved");
    } catch {
      message.error("Failed to save YouTube config");
    } finally {
      setYtSaving(false);
    }
  };

  const handleSchedSubmit = async (values: ScheduleConfig) => {
    setSchedSaving(true);
    try {
      await updateSchedule(values);
      setCurrentCron(values.cron_expression);
      setCurrentTimezone(values.timezone);
      message.success("Schedule updated");
    } catch {
      message.error("Failed to update schedule");
    } finally {
      setSchedSaving(false);
    }
  };

  if (loading) return <div style={{ textAlign: "center", padding: 40 }}><Spin size="large" data-testid="spinner" /></div>;

  return (
    <div className="fade-in-up">
      <Title level={3}>
        <YoutubeOutlined /> Settings
      </Title>

      <Card title={<><ClockCircleOutlined /> Schedule</>} style={{ maxWidth: 600, marginBottom: 16 }}>
        <Tag icon={<GlobalOutlined />} style={{ marginBottom: 12 }}>{currentTimezone} · {currentCron}</Tag>
        <Form
          form={schedForm}
          layout="vertical"
          onFinish={handleSchedSubmit}
          initialValues={{ cron_expression: "0 * * * *", timezone: "UTC" }}
        >
          <Form.Item label="Presets" style={{ marginBottom: 8 }}>
            <Select
              allowClear
              placeholder="Select a preset..."
              style={{ width: "100%" }}
              options={CRON_PRESETS}
              onChange={(value) => {
                if (value) schedForm.setFieldsValue({ cron_expression: value });
              }}
            />
          </Form.Item>
          <Form.Item
            name="cron_expression"
            label="Cron Expression"
            rules={[{ required: true, message: "Enter a cron expression" }]}
            help={
              <span>
                Five-field cron syntax.{" "}
                <a href="https://crontab.guru/" target="_blank" rel="noopener noreferrer">
                  <LinkOutlined /> crontab.guru
                </a>
              </span>
            }
          >
            <Input placeholder="0 */6 * * *" />
          </Form.Item>
          <Form.Item
            name="timezone"
            label="Timezone"
            rules={[{ required: true, message: "Enter a timezone" }]}
            help="e.g. UTC, Europe/Madrid"
          >
            <Input placeholder="UTC" />
          </Form.Item>
          <Button type="primary" htmlType="submit" loading={schedSaving} icon={<SaveOutlined />}>
            Save Schedule
          </Button>
        </Form>
      </Card>

      <Card title={<><YoutubeOutlined /> YouTube API Key</>} style={{ maxWidth: 600 }}>
        <Alert
          message="A YouTube Data API v3 key is needed to fetch videos and resolve @handles to channel IDs. Get one at https://console.cloud.google.com/apis/credentials"
          type="info"
          showIcon
          icon={<InfoCircleOutlined />}
          style={{ marginBottom: 20 }}
        />
        <Form
          form={ytForm}
          layout="vertical"
          onFinish={handleYtSubmit}
          initialValues={{ api_key: "" }}
        >
          <Form.Item
            name="api_key"
            label="YouTube Data API Key"
            rules={[{ required: true, message: "API key is required for YouTube feeds" }]}
          >
            <Input.Password placeholder="AIzaSy..." />
          </Form.Item>
          <Button type="primary" htmlType="submit" loading={ytSaving} icon={<SaveOutlined />}>
            Save
          </Button>
        </Form>
      </Card>
    </div>
  );
}