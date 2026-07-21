import { useEffect, useState } from "react";
import {
  Table, Button, Modal, Form, Input, Select, Switch, Typography, Space, Tag, message, Popconfirm,
} from "antd";
import { PlusOutlined } from "@ant-design/icons";
import {
  fetchPublishers, createPublisher, updatePublisher, testPublisher, deletePublisher, togglePublisher, getOAuthUrl,
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
    { name: "access_token", label: "Access Token", isPassword: true },
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
    { name: "access_token", label: "Access Token", isPassword: true },
    { name: "user_id", label: "User ID" },
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
  const [testing, setTesting] = useState(false);
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

  const handleToggle = async (id: string) => {
    try {
      const { enabled } = await togglePublisher(id);
      setPublishers(prev => ({
        ...prev,
        [id]: { ...prev[id], enabled },
      }));
    } catch (e) {
      const msg = e instanceof Error ? e.message : "Unknown error";
      message.error(`Failed to toggle: ${msg}`);
    }
  };

  const handleOAuth = async (id: string, type: string) => {
    try {
      const { url } = await getOAuthUrl(id);
      const origin = window.location.origin;
      const callbackUrl = `${origin}/oauth/callback?publisher_id=${id}`;
      const authUrl = new URL(url);
      authUrl.searchParams.set("redirect_uri", callbackUrl);
      const popup = window.open(
        authUrl.toString(),
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

  const handleTest = async () => {
    if (!editingId) return;
    setTesting(true);
    try {
      const result = await testPublisher(editingId);
      message.success(`✅ Test OK: ${result.message}`);
    } catch (e) {
      const msg = e instanceof Error ? e.message : "Unknown error";
      message.error(`❌ Test failed: ${msg}`);
    } finally {
      setTesting(false);
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
        <Switch
          checked={record.config.enabled}
          onChange={() => handleToggle(record.id)}
          size="small"
        />
      ),
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
                : (
                  <Button size="small" type="primary" onClick={() => handleOAuth(record.id, type)}>
                    Connect
                  </Button>
                )
            )}
            <Button size="small" onClick={() => handleEdit(record.id, record.config)}>Edit</Button>
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
            {editingId && (
              <Button onClick={handleTest} loading={testing} danger>
                Test
              </Button>
            )}
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

          <Form.Item name={["config", "template"]} label="Template (optional)">
            <Input.TextArea rows={3} placeholder="Custom template for this publisher" />
          </Form.Item>
        </Form>
      </Modal>
    </div>
  );
}
