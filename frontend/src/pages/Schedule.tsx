import { useEffect, useState } from "react";
import { Card, Form, Input, InputNumber, Button, Typography, message, Spin } from "antd";
import { fetchSchedule, updateSchedule, type ScheduleConfig } from "../api/http";

const { Title } = Typography;

export default function Schedule() {
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [form] = Form.useForm();

  useEffect(() => {
    fetchSchedule()
      .then((data) => form.setFieldsValue(data))
      .catch((e) => message.error("Failed to load schedule"))
      .finally(() => setLoading(false));
  }, [form]);

  const handleSubmit = async (values: ScheduleConfig) => {
    setSaving(true);
    try {
      await updateSchedule(values);
      message.success("Schedule updated");
    } catch (e) {
      message.error("Failed to update schedule");
    } finally {
      setSaving(false);
    }
  };

  if (loading) return <div style={{ textAlign: "center", padding: 40 }}><Spin size="large" /></div>;

  return (
    <div>
      <Title level={3}>Schedule</Title>
      <Card style={{ maxWidth: 500 }}>
        <Form form={form} layout="vertical" onFinish={handleSubmit}>
          <Form.Item name="default_interval_minutes" label="Default Check Interval (minutes)"
            rules={[{ required: true }]}>
            <InputNumber min={1} max={1440} style={{ width: "100%" }} />
          </Form.Item>
          <Form.Item name="timezone" label="Timezone" rules={[{ required: true }]}>
            <Input placeholder="UTC" />
          </Form.Item>
          <Button type="primary" htmlType="submit" loading={saving}>Save</Button>
        </Form>
      </Card>
    </div>
  );
}