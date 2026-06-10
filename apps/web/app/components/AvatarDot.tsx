// One presence/identity chip (cf-presence__dot) for the whole console: the
// SV-cast comic avatar when the principal has one (lib/presence avatarOf),
// the tag/initials fallback on the identity color otherwise.

import { avatarOf } from "@/lib/presence";

export function AvatarDot({
  id,
  fallback,
  color,
  live = false,
  title,
  stacked = false,
}: {
  id: string;
  /** Text shown when no cast avatar exists (tag or initials). */
  fallback: string;
  color: string;
  live?: boolean;
  title?: string;
  /** Overlapping facepile in the topbar; standalone chips reset the margin. */
  stacked?: boolean;
}) {
  const src = avatarOf(id);
  return (
    <span
      className={`cf-presence__dot${live ? " cf-presence__dot--live" : ""}`}
      style={{ background: color, ...(stacked ? {} : { marginLeft: 0 }) }}
      title={title}
    >
      {src ? <img src={src} alt="" /> : fallback}
    </span>
  );
}
