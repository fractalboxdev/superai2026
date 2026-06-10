import { Outlet } from "react-router";
import { AppSidebar } from "@/components/AppSidebar";
import { useAccess } from "@/lib/accessStore";

/** App shell: the permanent left sidebar wraps every surface. */
export default function Shell() {
  const { requests } = useAccess();
  const pending = requests.filter((r) => r.status === "pending").length;
  return (
    <div className="shell">
      <AppSidebar inboxCount={pending} />
      <div className="shell__content">
        <Outlet />
      </div>
    </div>
  );
}
