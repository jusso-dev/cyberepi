export default function OrganisationsPage() {
  const templates = ["Micro business", "Small business", "Medium enterprise", "Large enterprise", "Government agency", "Cloud-native startup", "Hybrid enterprise", "Software company", "University", "Managed service provider"];
  return (
    <section className="max-w-3xl">
      <h1 className="text-4xl">Populations</h1>
      <p className="mt-2 text-muted">Templates generate fictional organisations. They are not inventories of a real estate.</p>
      <ul className="mt-8 columns-1 gap-8 md:columns-2">
        {templates.map((name) => <li key={name} className="mb-3 border-b border-line py-2">{name}</li>)}
      </ul>
    </section>
  );
}
