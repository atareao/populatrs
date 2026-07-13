import { useEffect, useState } from "react";
import { Card, Form, Input, Button, Typography, message, Spin } from "antd";
import { fetchStorage, updateStorage, type StorageConfig } from "../api/http";

const { Title } = Typography;

export default function Settings() {
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [form] = Form.useForm();

  useEffect(() => {
    fetchStorage()
      .then((data) => form.setFieldsValue(data))
      .catch(() => message.error("Failed to load storage config"))
      .finally(() => setLoading(false));
  }, [form]);

  const handleSubmit = async (values: StorageConfig) => {
    setSaving(true);
    try {
      await updateStorage(values);
      message.success("Storage config updated");
    } catch {
      message.error("Failed to update storage config");
    } finally {
      setSaving(false);
    }
  };

  if (loading) return <div style={{ textAlign: "center", padding: 40 }}><Spin size="large" /></div>;

  return (
    <div>
      <Title level={3}>Settings</Title>
      <Card title="Storage" style={{ maxWidth: 500 }}>
        <Form form={form} layout="vertical" onFinish={handleSubmit}>
          <Form.Item name="data_dir" label="Data Directory" rules={[{ required: true }]}>
            <Input placeholder="./data" />
          </Form.Item>
          <Form.Item name="published_posts_file" label="Published Posts File" rules={[{ required: true }]}>
            <Input placeholder="published_posts.json" />
          </Form.Item>
          <Button type="primary" htmlType="submit" loading={saving}>Save</Button>
        </Form>
      </Card>
    </div>
  );
}