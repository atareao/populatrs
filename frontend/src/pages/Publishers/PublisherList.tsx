import { useEffect, useState } from "react";
import {
  Table, Button, Modal, Form, Input, Select, Switch, Typography, Space, Tag, message, Popconfirm,
} from "antd";
import { PlusOutlined } from "@ant-design/icons";
import {
  fetchPublishers, createPublisher, updatePublisher, testPublisher, deletePublisher, getOAuthUrl,
  type PublisherConfigEntry,
} from "../../api/http";

const { Title } = Typography;

const PUBLISHER_TYPES = [
  "Telegram", "X", "Mastodon", "LinkedIn", "Matrix", "Bluesky", "Threads", "Discord", "OpenObserve",
];

/** Fields shown for each publisher type — maps type name to antd form field configs. */
const TYPE_FIELDS: Record<string, { name: string; label: string; isPassword?: boolean }[]> = {
  Telegram: [
    { name: "bot_token", label: "Bot Token", isPassword: true },
    { name: "chat_id", label: "Chat ID" },
    { name: "parse_mode", label: "Parse Mode" },
    { name: "message_thread_id", label: "Message Thread ID" },
  ],
  X: [
    { name: "client_id", label: "Client ID" },
    { name: "client_secret", label: "Client Secret", isPassword: true },
    { name: "access_token", label: "Access Token", isPassword: true },
    { name: "refresh_token", label: "Refresh Token", isPassword: true },
    { name: "redirect_uri", label: "Redirect URI" },
  ],
  Mastodon: [
    { name: "server_url", label: "Server URL" },
    { name: "client_id", label: "Client ID" },
    { name: "client_secret", label: "Client Secret", isPassword: true },
    { name: "access_token", label: "Access Token", isPassword: true },
    { name: "redirect_uri", label: "Redirect URI" },
  ],
  LinkedIn: [
    { name: "client_id", label: "Client ID" },
    { name: "client_secret", label: "Client Secret", isPassword: true },
    { name: "access_token", label: "Access Token", isPassword: true },
    { name: "refresh_token", label: "Refresh Token", isPassword: true },
    { name: "user_id", label: "User ID" },
    { name: "redirect_uri", label: "Redirect URI" },
  ],
  Matrix: [
    { name: "homeserver_url", label: "Homeserver URL" },
    { name: "access_token", label: "Access Token", isPassword: true },
    { name: "room_id", label: "Room ID" },
  ],
  Bluesky: [
    { name: "handle", label: "Handle" },
    { name: "password", label: "Password", isPassword: true },
    { name: "pds_url", label: "PDS URL" },
  ],
  Threads: [
    { name: "client_id", label: "Client ID" },
    { name: "client_secret", label: "Client Secret", isPassword: true },
    { name: "access_token", label: "Access Token", isPassword: true },
    { name: "user_id", label: "User ID" },
    { name: "redirect_uri", label: "Redirect URI" },
  ],
  Discord: [
    { name: "webhook_url", label: "Webhook URL" },
  ],
  OpenObserve: [
    { name: "url", label: "URL" },
    { name: "organization", label: "Organization" },
    { name: "stream_name", label: "Stream Name" },
    { name: "access_token", label: "Access Token", isPassword: true },
  ],
};

export default function PublisherList() {
  const [publishers, setPublishers] = useState<Record<string, PublisherConfigEntry>>({});
  const [loading, setLoading] = useState(true);
  const [modalOpen, setModalOpen] = useState(false);
  const [editingId, setEditingId] = useState<string | null>(null);
  const [selectedType, setSelectedType] = useState<string>("Telegram");
  const [testingId, setTestingId] = useState<string | null>(null);
  const [testResultVisible, setTestResultVisible] = useState(false);
  const [testResult, setTestResult] = useState<{ success: boolean; message: string } | null>(null);
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

  const handleCreate = () => {
    setEditingId(null);
    setSelectedType("Telegram");
    form.resetFields();
    form.setFieldsValue({ type: "Telegram" });
    setModalOpen(true);
  };

  const handleEdit = (id: string, config: PublisherConfigEntry) => {
    setEditingId(id);
    setSelectedType(config.type);
    form.setFieldsValue({
      id,
      type: config.type,
      config: config.config,
      enabled: config.enabled,
    });
    setModalOpen(true);
  };

  const handleOAuth = async (id: string, type: string) => {
    try {
      const { url } = await getOAuthUrl(id);
      const popup = window.open(
        url,
        `oauth-${type}`,
        "width=800,height=700,scrollbars=yes",
      );
      if (!popup) {
        message.error("Popup blocked. Please allow popups for this site.");
      }
    } catch (e) {
      message.error(`Failed to start OAuth: ${e instanceof Error ? e.message : "Unknown error"}`);
    }
  };

  // Listen for OAuth popup messages (backend sends postMessage on success/error)
  useEffect(() => {
    const handler = (event: MessageEvent) => {
      if (event.data?.type === "oauth-success" || event.data?.type === "oauth-error") {
        loadData();
        if (event.data.type === "oauth-success") {
          message.success("OAuth completed successfully!");
        } else {
          message.error("OAuth failed");
        }
      }
    };
    window.addEventListener("message", handler);
    return () => window.removeEventListener("message", handler);
  }, []);

  const handleSubmit = async () => {
    try {
      const values = await form.validateFields();
      const config: PublisherConfigEntry = {
        type: values.type,
        config: values.config,
        enabled: values.enabled ?? true,
      };

      if (editingId) {
        await updatePublisher(editingId, config);
        message.success("Publisher updated");
      } else {
        await createPublisher(values.id, config);
        message.success("Publisher created");
      }
      setModalOpen(false);
      loadData();
    } catch (e) {
      if (e instanceof Error) message.error(e.message);
    }
  };

  const handleDelete = async (id: string) => {
    try {
      await deletePublisher(id);
      message.success("Publisher deleted");
      loadData();
    } catch (e) {
      const msg = e instanceof Error ? e.message : "Unknown error";
      message.error(`Failed to delete: ${msg}`);
    }
  };

  const handleTestFromTable = async (id: string) => {
    setTestingId(id);
    try {
      const result = await testPublisher(id);
      setTestResult({ success: true, message: result.message });
    } catch (e) {
      const msg = e instanceof Error ? e.message : "Unknown error";
      setTestResult({ success: false, message: msg });
    } finally {
      setTestingId(null);
      setTestResultVisible(true);
    }
  };

  /** Check if a publisher has all required fields to be considered "configured". */
  const isPublisherConfigured = (config: Record<string, any>, type: string): boolean => {
    switch (type) {
      case "Telegram":
        return !!(config.bot_token && config.chat_id);
      case "X":
        return !!(config.client_id && config.client_secret && config.access_token);
      case "Mastodon":
        return !!(config.server_url && config.access_token);
      case "LinkedIn":
        return !!(config.client_id && config.client_secret && config.access_token);
      case "OpenObserve":
        return !!(config.url && config.organization && config.stream_name && config.access_token);
      case "Matrix":
        return !!(config.homeserver_url && config.access_token && config.room_id);
      case "Bluesky":
        return !!(config.handle && config.password);
      case "Threads":
        return !!(config.client_id && config.client_secret && config.access_token);
      case "Discord":
        return !!(config.webhook_url);
      default:
        return false;
    }
  };

  // Reset form fields when type changes
  const handleTypeChange = (type: string) => {
    setSelectedType(type);
    // Clear previous config fields but keep id and type
    const id = form.getFieldValue("id");
    form.resetFields();
    form.setFieldsValue({ id, type });
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
      title: "Enabled", key: "enabled",
      render: (_: unknown, record: { id: string; config: PublisherConfigEntry }) => (
        <span>{record.config.enabled ? "✅" : "❌"}</span>
      ),
    },
    {
      title: "Actions", key: "actions",
      render: (_: unknown, record: { id: string; config: PublisherConfigEntry }) => {
        const type = record.config.type;
        const cfg = record.config.config;
        const isOAuthType = type === "X" || type === "LinkedIn" || type === "Threads" || type === "Mastodon";
        const connected = !!(cfg.access_token);

        const configured = isPublisherConfigured(cfg, type);

        return (
          <Space>
            {isOAuthType && (
              connected
                ? <Tag color="green">Connected</Tag>
                : (
                  <Button size="small" type="primary" onClick={() => handleOAuth(record.id, type)}>
                    Connect
                  </Button>
                )
            )}
            <Button size="small" onClick={() => handleEdit(record.id, record.config)}>Edit</Button>
            <Button
              size="small"
              onClick={() => handleTestFromTable(record.id)}
              disabled={!configured}
              loading={testingId === record.id}
            >
              Test
            </Button>
            <Popconfirm
              title="Delete publisher"
              description="Are you sure you want to delete this publisher?"
              onConfirm={() => handleDelete(record.id)}
              okText="Yes, delete"
              cancelText="Cancel"
              okButtonProps={{ danger: true }}
            >
              <Button size="small" danger>Delete</Button>
            </Popconfirm>
          </Space>
        );
      },
    },
  ];

  const dataSource = Object.entries(publishers).map(([id, config]) => ({ id, config }));
  const fields = TYPE_FIELDS[selectedType] ?? [];
  const isCreating = editingId === null;

  return (
    <div>
      <div style={{ display: "flex", justifyContent: "space-between", marginBottom: 16 }}>
        <Title level={3}>Publishers</Title>
        <Button type="primary" icon={<PlusOutlined />} onClick={handleCreate}>Add Publisher</Button>
      </div>

      <Table dataSource={dataSource} columns={columns} rowKey="id" loading={loading} />

      <Modal
        title={editingId ? `Edit Publisher: ${editingId}` : "Create Publisher"}
        open={modalOpen}
        onOk={handleSubmit}
        onCancel={() => setModalOpen(false)}
        width={600}
        footer={(_, { OkBtn, CancelBtn }) => (
          <Space>
            <CancelBtn />
            <OkBtn />
          </Space>
        )}
      >
        <Form form={form} layout="vertical">
          {isCreating && (
            <Form.Item name="id" label="Publisher ID" rules={[{ required: true }]}>
              <Input placeholder="my-publisher" />
            </Form.Item>
          )}
          <Form.Item name="type" label="Type" rules={[{ required: true }]}>
            <Select onChange={handleTypeChange}>
              {PUBLISHER_TYPES.map(t => <Select.Option key={t} value={t}>{t}</Select.Option>)}
            </Select>
          </Form.Item>

          <Form.Item name="enabled" label="Enabled" valuePropName="checked">
            <Switch />
          </Form.Item>

          {fields.map(f => (
            <Form.Item key={f.name} name={["config", f.name]} label={f.label}>
              {f.isPassword ? <Input.Password /> : <Input />}
            </Form.Item>
          ))}

          <Form.Item
                name={["config", "template"]}
                label="Template"
                rules={[{ required: true, message: "Template is required" }]}
                extra="Use {{ title }}, {{ description }}, {{ url }} variables"
              >
                <Input.TextArea rows={3} placeholder="📰 *{{ title }}* - {{ url }}" />
              </Form.Item>
        </Form>
      </Modal>

      <Modal
        title={testResult?.success ? "✅ Test Successful" : "❌ Test Failed"}
        open={testResultVisible}
        onOk={() => setTestResultVisible(false)}
        onCancel={() => setTestResultVisible(false)}
        footer={[
          <Button key="ok" type="primary" onClick={() => setTestResultVisible(false)}>
            OK
          </Button>,
        ]}
      >
        <p>{testResult?.message}</p>
      </Modal>
    </div>
  );
}
