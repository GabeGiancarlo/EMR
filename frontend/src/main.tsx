import './brand/tokens.css';
/*
 * The staff workspace and the patient visit page are downloaded separately (see below), but
 * their stylesheets stay in this one bundle, in the order they have always had. Rules of
 * equal weight are decided by that order, so a stylesheet arriving later with its page
 * would change how things look.
 */
import './drafting.css';
import './knowledge.css';
import './clientRegistration.css';
import './intake.css';
import './programSettings.css';
import './calendar.css';
import './telehealth.css';
import './recording.css';
import './transcript.css';
import './workspace.css';
import './profileMenu.css';
import './signIn.css';
import './search.css';
import './styles.css';
import './workflows.css';
import './theme.css';
import './nexus-ui/nexus-ui.css';
import './nexus-ui/app.css';
import { StrictMode, Suspense, lazy } from 'react';
import { createRoot } from 'react-dom/client';
import AuthBoundary from './SignIn';
import { ErrorBoundary, PageLoading } from './nexus-ui';
import { applyTheme, readStoredTheme } from './theme';

applyTheme(readStoredTheme());

// A patient opening /visit downloads only the visit page, not the staff workspace.
const visit = window.location.pathname === '/visit';
const loadWorkspace = () => import('./App');
const loadVisit = () => import('./TelehealthVisit');
const App = lazy(loadWorkspace);
const TelehealthVisit = lazy(loadVisit);
// Start this page's download now instead of after sign-in settles. A failure here is
// reported when the page is drawn, by the boundary below.
void (visit ? loadVisit() : loadWorkspace()).catch(() => {});

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <ErrorBoundary>
      {visit ? (
        <Suspense fallback={<PageLoading label="Opening your visit…" />}>
          <TelehealthVisit />
        </Suspense>
      ) : (
        <AuthBoundary>
          <Suspense fallback={<PageLoading label="Connecting your workspace…" />}>
            <App />
          </Suspense>
        </AuthBoundary>
      )}
    </ErrorBoundary>
  </StrictMode>,
);
