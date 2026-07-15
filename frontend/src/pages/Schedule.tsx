import { useEffect, useState } from "react";
import { Card, Form, Input, InputNumber, Button, Typography, message, Spin, Alert, Descriptions, Tag } from "antd";
import { ClockCircleOutlined, GlobalOutlined, SaveOutlined, InfoCircleOutlined } from "@ant-design/icons";
import { fetchSchedule, updateSchedule, type ScheduleConfig } from "../api/http";

const { Title } = Typography;

export default function Schedule() {
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [currentInterval, setCurrentInterval] = useState<number>(60);
  const [currentTimezone, setCurrentTimezone] = useState<string>("UTC");
  const [form] = Form.useForm();

  useEffect(() => {
    fetchSchedule()
      .then((data) => {
        form.setFieldsValue(data);
        setCurrentInterval(data.default_interval_minutes);
        setCurrentTimezone(data.timezone);
      })
      .catch(() => message.error("Failed to load schedule"))
      .finally(() => setLoading(false));
  }, [form]);

  const handleSubmit = async (values: ScheduleConfig) => {
    setSaving(true);
    try {
      await updateSchedule(values);
      setCurrentInterval(values.default_interval_minutes);
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
          <Descriptions.Item label="Check Interval">{currentInterval} minutes</Descriptions.Item>
          <Descriptions.Item label="Timezone"><Tag icon={<GlobalOutlined />}>{currentTimezone}</Tag></Descriptions.Item>
        </Descriptions>
      </Card>

      {/* Edit form */}
      <Card title="Edit Schedule" style={{ maxWidth: 600 }}>
        <Alert
          message="The scheduler runs every N minutes checking all enabled feeds for new content."
          type="info"
          showIcon
          icon={<InfoCircleOutlined />}
          style={{ marginBottom: 20 }}
        />
        <Form
          form={form}
          layout="vertical"
          onFinish={handleSubmit}
          initialValues={{ default_interval_minutes: 60, timezone: "UTC" }}
        >
          <Form.Item
            name="default_interval_minutes"
            label="Default Check Interval (minutes)"
            rules={[{ required: true, message: "Please set an interval" }]}
            help="How often to check feeds for new posts (1-1440 min)"
          >
            <InputNumber min={1} max={1440} style={{ width: "100%" }} />
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