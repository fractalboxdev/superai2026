// Mermaid theme pinned to the landing design system (packages/design-system/
// tokens.css): indigo-subtle node cards, slate text/edges, teal-tinted
// clusters. Slides may still override individual nodes with `style …` lines
// (denied = danger red, trusted environment = comic teal) — those win.

// Like setup/main.ts: `defineMermaidSetup` from @slidev/types is an identity
// helper; export the plain function to avoid adding the dependency.
export default function setupMermaid() {
  return {
    theme: 'base' as const,
    themeVariables: {
      fontFamily: 'Inter, ui-sans-serif, system-ui, sans-serif',

      // nodes — indigo-50 fill, indigo-600 border, slate-900 text
      primaryColor: '#eef2ff',
      primaryTextColor: '#0f172a',
      primaryBorderColor: '#4f46e5',

      // secondary/tertiary — teal-100 comic tint, slate-50 surface
      secondaryColor: '#d8efe7',
      secondaryTextColor: '#0b3a33',
      secondaryBorderColor: '#14534a',
      tertiaryColor: '#f8fafc',
      tertiaryTextColor: '#334155',
      tertiaryBorderColor: '#94a3b8',

      // edges + labels
      lineColor: '#334155',
      textColor: '#0f172a',
      edgeLabelBackground: '#ffffff',

      // subgraphs
      clusterBkg: '#f8fafc',
      clusterBorder: '#94a3b8',
    },
  }
}
