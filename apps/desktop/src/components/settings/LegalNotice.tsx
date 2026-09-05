import { ExtLink } from "@/components/ui/external-link";

const TERMS_URL = "https://sitecmd.com/terms";
const PRIVACY_POLICY_URL = "https://sitecmd.com/privacy";

interface LegalNoticeProps {
  /** Completes "By <action>, you agree to", naming the step this sits beside. */
  action: string;
}

/** The assent line shown wherever a step binds the user to the hosted terms. */
export function LegalNotice({ action }: LegalNoticeProps) {
  return (
    <p className="body-muted">
      By {action}, you agree to the SiteCMD <ExtLink href={TERMS_URL}>Terms of Service</ExtLink> and{" "}
      <ExtLink href={PRIVACY_POLICY_URL}>Privacy Policy</ExtLink>.
    </p>
  );
}
