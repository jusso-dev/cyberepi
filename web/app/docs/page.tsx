export default function DocsPage() {
  return (
    <article className="max-w-3xl space-y-4">
      <h1 className="text-4xl">Notes</h1>
      <p>CyberEpi is a simulation. Transmission is a probability on a contact graph. It does not scan, exploit, or change a real network.</p>
      <p><strong>Attack rate.</strong> The share of simulated entities that became exposed or compromised during the outbreak.</p>
      <p><strong>R0.</strong> The average number of secondary compromises one compromised entity would cause in an otherwise susceptible population.</p>
      <p><strong>Rt.</strong> That same expectation given who is still susceptible now. Above 1 the outbreak is expanding. Below 1 it is contracting.</p>
      <p>Built-in control multipliers are illustrative simulation assumptions. Replace them when you have evidence of your own.</p>
    </article>
  );
}
