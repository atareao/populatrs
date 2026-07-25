import { useEffect, useState } from "react";
import { Card, Form, Input, Button, Typography, message, Spin, Alert } from "antd";
import { SaveOutlined, InfoCircleOutlined, YoutubeOutlined } from "@ant-design/icons";
import { fetchYoutubeConfig, updateYoutubeConfig } from "../api/http";

const { Title } = Typography;

export default function Settings() {
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [form] = Form.useForm();

  useEffect(() => {
    fetchYoutubeConfig()
      .then((data) => form.setFieldsValue(data))
      .catch(() => message.error("Failed to load config"))
      .finally(() => setLoading(false));
  }, [form]);

  const handleSubmit = async (values: { api_key: string }) => {
    setSaving(true);
    try {
      await updateYoutubeConfig(values);
      message.success("YouTube config saved");
    } catch {
      message.error("Failed to save YouTube config");
    } finally {
      setSaving(false);
    }
  };

  if (loading) return <div style={{ textAlign: "center", padding: 40 }}><Spin size="large" data-testid="spinner" /></div>;

  return (
    <div className="fade-in-up">
      <Title level={3}>
        <YoutubeOutlined /> Settings
      </Title>

      <Card title={<><YoutubeOutlined /> YouTube Configuration</>} style={{ maxWidth: 600 }}>
        <Alert
          message="A YouTube Data API v3 key is needed to fetch videos and resolve @handles to channel IDs. Get one at https://console.cloud.google.com/apis/credentials"
          type="info"
          showIcon
          icon={<InfoCircleOutlined />}
          style={{ marginBottom: 20 }}
        />
        <Form
          form={form}
          layout="vertical"
          onFinish={handleSubmit}
          initialValues={{ api_key: "" }}
        >
          <Form.Item
            name="api_key"
            label="YouTube Data API Key"
            rules={[{ required: true, message: "API key is required for YouTube feeds" }]}
          >
            <Input.Password placeholder="AIzaSy..." />
          </Form.Item>
          <Button type="primary" htmlType="submit" loading={saving} icon={<SaveOutlined />}>
            Save
          </Button>
        </Form>
      </Card>
    </div>
  );
}