import { useEffect, useState } from "react";
import {
  Table, Button, Modal, Form, Input, Select, Typography, Space, Tag, message,
} from "antd";
import { fetchPublishers, updatePublisher, getOAuthUrl, type PublisherConfigEntry } from "../../api/http";

const { Title } = Typography;

const PUBLISHER_TYPES = ["Telegram", "X", "Mastodon", "LinkedIn", "Matrix", "Bluesky", "Threads", "Discord", "OpenObserve"];

export default function PublisherList() {
  const [publishers, setPublishers] = useState<Record<string, PublisherConfigEntry>>({});
  const [loading, setLoading] = useState(true);
  const [modalOpen, setModalOpen] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [form] = Form.useForm();

  const loadData = async () => {
    try {
      const data = await fetchPublishers();
      setPublishers(data.publishers);
    } catch (e) {
      message.error("Failed to load publishers");
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => { loadData(); }, []);

  const handleEdit = (id: string, config: PublisherConfigEntry) => {
    setEditingId(id);
    form.setFieldsValue({
      type: config.type,
      config: config.config,
    });
    setModalOpen(true);
  };

  const handleOAuth = async (id: string, type: string) => {
    try {
      const { url } = await getOAuthUrl(id);
      const origin = window.location.origin;
      const callbackUrl = `${origin}/oauth/callback?publisher_id=${id}`;
      // Append redirect_uri to the auth URL
      const authUrl = new URL(url);
      authUrl.searchParams.set("redirect_uri", callbackUrl);
      // Open popup window
      const popup = window.open(
        authUrl.toString(),
        `oauth-${type}`,
        "width=800,height=700,scrollbars=yes"
      );
      if (!popup) {
        message.error("Popup blocked. Please allow popups for this site.");
      }
    } catch (e) {
      message.error(`Failed to start OAuth: ${e instanceof Error ? e.message : "Unknown error"}`);
    }
  };

  const handleSubmit = async () => {
    if (!editingId) return;
    try {
      const values = await form.validateFields();
      await updatePublisher(editingId, { type: values.type, config: values.config });
      message.success("Publisher updated");
      setModalOpen(false);
      loadData();
    } catch (e) {
      if (e instanceof Error) message.error(e.message);
    }
  };

  const columns = [
    { title: "Publisher ID", dataIndex: "id", key: "id" },
    {
      title: "Type", key: "type",
      render: (_: unknown, record: { id: string; config: PublisherConfigEntry }) => (
        <Tag color="blue">{record.config.type}</Tag>
      ),
    },
    {
      title: "Status", key: "status",
      render: (_: unknown, record: { id: string; config: PublisherConfigEntry }) => {
        const cfg = record.config.config;
        const hasToken = cfg.access_token || cfg.bot_token || cfg.webhook_url;
        return hasToken ? <Tag color="green">Configured</Tag> : <Tag color="orange">Pending</Tag>;
      },
    },
    {
      title: "Actions", key: "actions",
      render: (_: unknown, record: { id: string; config: PublisherConfigEntry }) => {
        const type = record.config.type;
        const cfg = record.config.config;
        const isOAuthType = type === "X" || type === "LinkedIn";
        const connected = !!(cfg.access_token);

        return (
          <Space>
            {isOAuthType && (
              connected
                ? <Tag color="green">Connected</Tag>
                : <Button size="small" type="primary" onClick={() => handleOAuth(record.id, type)}>
                    Connect
                  </Button>
            )}
            <Button size="small" onClick={() => handleEdit(record.id, record.config)}>Edit</Button>
          </Space>
        );
      },
    },
  ];

  const dataSource = Object.entries(publishers).map(([id, config]) => ({ id, config }));

  return (
    <div>
      <Title level={3}>Publishers</Title>
      <Table dataSource={dataSource} columns={columns} rowKey="id" loading={loading} />

      <Modal
        title={`Edit Publisher: ${editingId}`}
        open={modalOpen}
        onOk={handleSubmit}
        onCancel={() => setModalOpen(false)}
        width={600}
      >
        <Form form={form} layout="vertical">
          <Form.Item name="type" label="Type">
            <Select>
              {PUBLISHER_TYPES.map(t => <Select.Option key={t} value={t}>{t}</Select.Option>)}
            </Select>
          </Form.Item>
          <Form.Item name={["config", "bot_token"]} label="Bot Token">
            <Input.Password />
          </Form.Item>
          <Form.Item name={["config", "chat_id"]} label="Chat ID">
            <Input />
          </Form.Item>
          <Form.Item name={["config", "client_id"]} label="Client ID">
            <Input />
          </Form.Item>
          <Form.Item name={["config", "client_secret"]} label="Client Secret">
            <Input.Password />
          </Form.Item>
          <Form.Item name={["config", "access_token"]} label="Access Token">
            <Input.Password />
          </Form.Item>
          <Form.Item name={["config", "server_url"]} label="Server URL">
            <Input />
          </Form.Item>
          <Form.Item name={["config", "webhook_url"]} label="Webhook URL">
            <Input />
          </Form.Item>
          <Form.Item name={["config", "template"]} label="Template">
            <Input.TextArea rows={3} />
          </Form.Item>
        </Form>
      </Modal>
    </div>
  );
}