import { lazy, Suspense } from "react";
import { Routes, Route, Navigate } from "react-router-dom";
import { useAuth } from "./hooks/useAuth";
import AppLayout from "./components/AppLayout";
import { Spin } from "antd";

const LoginPage = lazy(() => import("./pages/LoginPage"));
const Dashboard = lazy(() => import("./pages/Dashboard"));
const FeedList = lazy(() => import("./pages/Feeds/FeedList"));
const PublisherList = lazy(() => import("./pages/Publishers/PublisherList"));
const OAuthCallback = lazy(() => import("./pages/OAuthCallback"));
const Schedule = lazy(() => import("./pages/Schedule"));
const Settings = lazy(() => import("./pages/Settings"));

function ProtectedRoute({ children }: { children: React.ReactNode }) {
  const { user, loading } = useAuth();

  if (loading) {
    return (
      <div style={{ display: "flex", justifyContent: "center", alignItems: "center", height: "100vh" }}>
        <Spin size="large" />
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
        <Route path="schedule" element={<SuspenseWrapper><Schedule /></SuspenseWrapper>} />
        <Route path="settings" element={<SuspenseWrapper><Settings /></SuspenseWrapper>} />
      </Route>
    </Routes>
  );
}