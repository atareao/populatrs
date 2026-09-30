import { lazy, Suspense } from "react";
import { Routes, Route, Navigate, useNavigate } from "react-router";
import { useAuth } from "./hooks/useAuth";
import AppLayout from "./components/AppLayout";
import { clearToken } from "./store/auth";
import { Button, Result, Spin } from "antd";

const LoginPage = lazy(() => import("./pages/LoginPage"));
const LogsPage = lazy(() => import("./pages/LogsPage"));
const Dashboard = lazy(() => import("./pages/Dashboard"));
const FeedList = lazy(() => import("./pages/Feeds/FeedList"));
const PublisherList = lazy(() => import("./pages/Publishers/PublisherList"));
const OAuthCallback = lazy(() => import("./pages/OAuthCallback"));
const Settings = lazy(() => import("./pages/Settings"));

function ProtectedRoute({ children }: { children: React.ReactNode }) {
  const { user, loading, error, retry } = useAuth();
  const navigate = useNavigate();

  if (loading) {
    return (
      <div style={{ display: "flex", justifyContent: "center", alignItems: "center", height: "100vh" }}>
        <Spin size="large" />
      </div>
    );
  }

  // Transient failure (network/server): do NOT redirect to /login, which would
  // loop between the login page and the protected area. Offer a retry instead,
  // plus an escape hatch so a persistently unreachable backend cannot trap the
  // user.
  if (error) {
    return (
      <div style={{ display: "flex", justifyContent: "center", alignItems: "center", height: "100vh" }}>
        <Result
          status="warning"
          title="No se pudo verificar la sesión"
          subTitle="Comprueba tu conexión e inténtalo de nuevo."
          extra={[
            <Button type="primary" key="retry" onClick={retry}>
              Reintentar
            </Button>,
            <Button
              key="login"
              onClick={() => {
                clearToken();
                navigate("/login", { replace: true });
              }}
            >
              Ir a iniciar sesión
            </Button>,
          ]}
        />
      </div>
    );
  }

  if (!user) {
    return <Navigate to="/login" replace />;
  }

  return <>{children}</>;
}

function SuspenseWrapper({ children }: { children: React.ReactNode }) {
  return (
    <Suspense fallback={
      <div style={{ display: "flex", justifyContent: "center", alignItems: "center", height: "60vh" }}>
        <Spin />
      </div>
    }>
      <div className="fade-in-up">{children}</div>
    </Suspense>
  );
}

export default function App() {
  return (
    <Routes>
      <Route path="/login" element={<SuspenseWrapper><LoginPage /></SuspenseWrapper>} />
      <Route path="/oauth/callback" element={<SuspenseWrapper><OAuthCallback /></SuspenseWrapper>} />
      <Route
        path="/"
        element={
          <ProtectedRoute>
            <AppLayout />
          </ProtectedRoute>
        }
      >
        <Route index element={<SuspenseWrapper><Dashboard /></SuspenseWrapper>} />
        <Route path="feeds" element={<SuspenseWrapper><FeedList /></SuspenseWrapper>} />
        <Route path="publishers" element={<SuspenseWrapper><PublisherList /></SuspenseWrapper>} />
        <Route path="settings" element={<SuspenseWrapper><Settings /></SuspenseWrapper>} />
        <Route path="logs" element={<SuspenseWrapper><LogsPage /></SuspenseWrapper>} />
      </Route>
    </Routes>
  );
}