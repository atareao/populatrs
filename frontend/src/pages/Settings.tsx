import { useEffect, useState } from "react";
import { Card, Form, Input, Button, Typography, message, Spin, Descriptions, Tag, Alert } from "antd";
import { DatabaseOutlined, SaveOutlined, InfoCircleOutlined, FolderOutlined, FileTextOutlined } from "@ant-design/icons";
import { fetchStorage, updateStorage, type StorageConfig } from "../api/http";

const { Title } = Typography;

export default function Settings() {
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [currentDataDir, setCurrentDataDir] = useState<string>("");
  const [currentPostsFile, setCurrentPostsFile] = useState<string>("");
  const [form] = Form.useForm();

  useEffect(() => {
    fetchStorage()
      .then((data) => {
        form.setFieldsValue(data);
        setCurrentDataDir(data.data_dir);
        setCurrentPostsFile(data.published_posts_file);
      })
      .catch(() => message.error("Failed to load storage config"))
      .finally(() => setLoading(false));
  }, [form]);

  const handleSubmit = async (values: StorageConfig) => {
    setSaving(true);
    try {
      await updateStorage(values);
      setCurrentDataDir(values.data_dir);
      setCurrentPostsFile(values.published_posts_file);
      message.success("Storage config saved");
    } catch {
      message.error("Failed to save storage config");
    } finally {
      setSaving(false);
    }
  };

  if (loading) return <div style={{ textAlign: "center", padding: 40 }}><Spin size="large" data-testid="spinner" /></div>;

  return (
    <div className="fade-in-up">
      <Title level={3}>
        <DatabaseOutlined /> Settings
      </Title>

      {/* Current configuration */}
      <Card style={{ marginBottom: 16, maxWidth: 600 }}>
        <Descriptions title="Current Configuration" column={1} size="small">
          <Descriptions.Item label={<><FolderOutlined /> Data Directory</>}>
            <code>{currentDataDir}</code>
          </Descriptions.Item>
          <Descriptions.Item label={<><FileTextOutlined /> Published Posts File</>}>
            <code>{currentPostsFile}</code>
          </Descriptions.Item>
        </Descriptions>
      </Card>

      {/* Edit form */}
      <Card title="Edit Storage Config" style={{ maxWidth: 600 }}>
        <Alert
          message="Storage paths are configured via environment variables. Changes here will apply after a server restart."
          type="warning"
          showIcon
          icon={<InfoCircleOutlined />}
          style={{ marginBottom: 20 }}
        />
        <Form
          form={form}
          layout="vertical"
          onFinish={handleSubmit}
          initialValues={{ data_dir: "./data", published_posts_file: "published_posts.json" }}
        >
          <Form.Item
            name="data_dir"
            label="Data Directory"
            rules={[{ required: true, message: "Please enter the data directory path" }]}
            help="Path where data files are stored (e.g. ./data, /app/data)"
          >
            <Input placeholder="./data" />
          </Form.Item>
          <Form.Item
            name="published_posts_file"
            label="Published Posts File"
            rules={[{ required: true, message: "Please enter the filename" }]}
            help="File name for tracking already-published posts"
          >
            <Input placeholder="published_posts.json" />
          </Form.Item>
          <Button type="primary" htmlType="submit" loading={saving} icon={<SaveOutlined />}>
            Save Changes
          </Button>
        </Form>
      </Card>
    </div>
  );
}