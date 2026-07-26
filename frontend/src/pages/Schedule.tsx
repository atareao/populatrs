import { useEffect, useState } from "react";
import { Card, Form, Input, Select, Button, Typography, message, Spin, Alert, Descriptions, Tag } from "antd";
import { ClockCircleOutlined, GlobalOutlined, SaveOutlined, InfoCircleOutlined, LinkOutlined } from "@ant-design/icons";
import { fetchSchedule, updateSchedule, type ScheduleConfig } from "../api/http";

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

export default function Schedule() {
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [currentCron, setCurrentCron] = useState<string>("0 * * * *");
  const [currentTimezone, setCurrentTimezone] = useState<string>("UTC");
  const [form] = Form.useForm();

  useEffect(() => {
    fetchSchedule()
      .then((data) => {
        form.setFieldsValue(data);
        setCurrentCron(data.cron_expression);
        setCurrentTimezone(data.timezone);
      })
      .catch(() => message.error("Failed to load schedule"))
      .finally(() => setLoading(false));
  }, [form]);

  const handleSubmit = async (values: ScheduleConfig) => {
    setSaving(true);
    try {
      await updateSchedule(values);
      setCurrentCron(values.cron_expression);
      setCurrentTimezone(values.timezone);
      message.success("Schedule updated successfully");
    } catch {
      message.error("Failed to update schedule");
    } finally {
      setSaving(false);
    }
  };

  if (loading) return <div style={{ textAlign: "center", padding: 40 }}><Spin size="large" data-testid="spinner" /></div>;

  return (
    <div className="fade-in-up">
      <Title level={3}>
        <ClockCircleOutlined /> Schedule
      </Title>

      {/* Current configuration summary */}
      <Card style={{ marginBottom: 16, maxWidth: 600 }}>
        <Descriptions title="Current Configuration" column={2} size="small">
          <Descriptions.Item label="Cron Expression">{currentCron}</Descriptions.Item>
          <Descriptions.Item label="Timezone"><Tag icon={<GlobalOutlined />}>{currentTimezone}</Tag></Descriptions.Item>
        </Descriptions>
      </Card>

      {/* Edit form */}
      <Card title="Edit Schedule" style={{ maxWidth: 600 }}>
        <Alert
          message="The scheduler checks feeds according to the cron expression."
          type="info"
          showIcon
          icon={<InfoCircleOutlined />}
          style={{ marginBottom: 20 }}
        />
        <Form
          form={form}
          layout="vertical"
          onFinish={handleSubmit}
          initialValues={{ cron_expression: "0 * * * *", timezone: "UTC" }}
        >
          <Form.Item
            name="cron_expression"
            label="Cron Expression"
            rules={[{ required: true, message: "Please enter a cron expression" }]}
            help={
              <span>
                Five-field cron syntax (minute hour dom month dow).{" "}
                <a href="https://crontab.guru/" target="_blank" rel="noopener noreferrer">
                  <LinkOutlined /> crontab.guru
                </a>
              </span>
            }
          >
            <Input placeholder="0 */6 * * *" style={{ width: "100%" }} />
          </Form.Item>
          <Form.Item label="Presets" style={{ marginBottom: 16 }}>
            <Select
              allowClear
              placeholder="Select a preset..."
              style={{ width: "100%" }}
              options={CRON_PRESETS}
              onChange={(value) => {
                if (value) form.setFieldsValue({ cron_expression: value });
              }}
            />
          </Form.Item>
          <Form.Item
            name="timezone"
            label="Timezone"
            rules={[{ required: true, message: "Please enter a timezone" }]}
            help="e.g. UTC, Europe/Madrid, America/New_York"
          >
            <Input placeholder="UTC" />
          </Form.Item>
          <Button type="primary" htmlType="submit" loading={saving} icon={<SaveOutlined />}>
            Save Changes
          </Button>
        </Form>
      </Card>
    </div>
  );
}