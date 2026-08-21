import { AppContent } from "@/app/AppContent";
import { AppProviders } from "@/app/AppProviders";

function App() {
  return (
    <AppProviders>
      {({ scanHook, historyHook }) => <AppContent scanHook={scanHook} historyHook={historyHook} />}
    </AppProviders>
  );
}

export default App;
