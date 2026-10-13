# Research Notes and References

This document catalogs external sources, standards, regulations, and academic research consulted during the design of Always Together. It is organized by topic area to support validation of architectural decisions and compliance claims.

---

## Research areas resolved/tracked before implementation

- OpenStreetMap attribution, tile usage, geocoding, and routing-provider
  licenses.
- Android and iPhone background-location policies and review requirements.
- Privacy law, data deletion, consent, children's safety, and
  emergency-service disclaimers.
- Geofencing accuracy, battery behavior, indoor positioning, and offline
  maps.
- Moderation design for random groups, creator groups, media, and calls.
- Secure messaging, notification privacy, and metadata minimization.
- Coding-prompt safety, sandboxed previews, and media/file handling if user
  submissions include executable or interactive content.
- Accessibility standards and device/network coverage.

---

## Sources consulted

### Privacy and Data Protection

- **GDPR (General Data Protection Regulation)**: <https://gdpr.eu/>
  - Articles 4(1), 6, 7, 17, 20, 32 — personal data definition, lawful basis, consent conditions, erasure, portability, security
  - **Relevance**: Defines legal basis for location sharing (Art. 6), explicit consent requirements (Art. 7), right to erasure (Art. 17), data portability (Art. 20), and security obligations (Art. 32)
  - **Implementation**: Directional consent records, expiry/revocation, deletion workflows, encryption at rest and in transit
  
- **ISO/IEC 27001**: <https://www.iso.org/isoiec-27001-information-security.html>
  - Information Security Management Systems; Controls A.9 (access control), A.10 (cryptography), A.12 (operations security), A.13 (communications security), A.18 (compliance)
  - **Relevance**: Provides systematic framework for information security management
  - **Implementation**: Security control register, access control policies, encryption key management, audit logging
  
- **ISO/IEC 27701**: <https://www.iso.org/standard/71670.html>
  - Privacy Information Management Extension (PII controller/processor requirements)
  - **Relevance**: Extends ISO 27001 with privacy-specific controls for PII processing
  - **Implementation**: Data minimization, purpose limitation, consent management
  
- **NIST Privacy Framework**: <https://www.nist.gov/privacy-framework>
  - Identify-Govern-Control-Communicate-Protect functions
  - **Relevance**: Risk-based approach to privacy engineering
  - **Implementation**: Privacy risk assessments, data classification, transparency mechanisms

### Mobile Security Standards

- **OWASP Mobile Application Security Verification Standard (MASVS)**: <https://owasp.org/www-project-mobile-application-security-verification-standard/>
  - V1–V8 control categories: Architecture, Data Storage, Cryptography, Authentication, Network, Platform, Code Quality, Resiliency
  - **Relevance**: Industry-standard mobile security verification criteria
  - **Implementation**: Secure token storage, certificate pinning, input validation, anti-tampering checks
  
- **OWASP Mobile Security Testing Guide (MSTG)**: <https://owasp.org/www-project-mobile-security-testing-guide/>
  - Comprehensive testing procedures for mobile applications
  - **Relevance**: Practical testing methodologies aligned with MASVS
  - **Implementation**: Security test cases, penetration testing checklist
  
- **OWASP API Security Top 10 (2023)**: <https://owasp.org/API-Security/editions/2023/en/0x-introduction/>
  - API1: Broken Object Level Authorization (BOLA)
  - API2: Broken Authentication
  - API3: Broken Object Property Authorization
  - API4: Unrestricted Resource Consumption
  - API5: Broken Function Level Authorization
  - API6: Unrestricted Access to Sensitive Business Flows
  - API7: Server Side Request Forgery (SSRF)
  - API8: Security Misconfiguration
  - API9: Improper Inventory Management
  - API10: Unsafe Consumption of APIs
  - **Relevance**: Directly applicable to location-sharing API design
  - **Implementation**: Authorization checks on every endpoint, rate limiting, input validation, secure error handling
  
- **Google Play Location Permissions Policy**: <https://support.google.com/googleplay/android-developer/answer/9799150>
  - Background location justification, just-in-time notifications, prominent disclosure requirements
  - **Relevance**: Mandatory for Android app distribution
  - **Implementation**: In-app permission explanations, background location toggle, notification indicators
  
- **Apple App Store Location Services Guidelines**: <https://developer.apple.com/app-store/review/guidelines/#location-services>
  - Usage description requirements, background location transparency, purpose limitation
  - **Relevance**: Mandatory for iOS app distribution
  - **Implementation**: NSLocationWhenInUseUsageDescription, NSLocationAlwaysUsageDescription strings, background mode justification

### Authentication and Authorization

- **OAuth 2.0 Security Best Current Practice (RFC 9700)**: <https://www.rfc-editor.org/rfc/rfc9700.html>
  - Updated security recommendations; PKCE requirement for public clients; authorization code flow mandatory
  - **Relevance**: Modern OAuth 2.0 security baseline for mobile apps
  - **Implementation**: Authorization code flow with PKCE S256, no implicit grant, short-lived access tokens
  
- **Proof Key for Code Exchange (PKCE) — RFC 7636**: <https://www.rfc-editor.org/rfc/rfc7636.html>
  - Authorization code interception prevention via code_verifier/code_challenge
  - **Relevance**: Prevents authorization code theft on mobile devices
  - **Implementation**: PKCE S256 for all OAuth flows
  
- **FIDO Alliance Passkeys**: <https://fidoalliance.org/passkeys/>
  - Phishing-resistant authentication via WebAuthn/FIDO2
  - **Relevance**: Passwordless authentication with strong security guarantees
  - **Implementation**: Planned for production authentication (deferred from MVP)

### Mapping and Location

- **OpenStreetMap Foundation, Tile Usage Policy**: <https://operations.osmfoundation.org/policies/tiles/>
  - Rate limits, attribution requirements, commercial use restrictions
  - **Relevance**: Legal compliance for map tile usage
  - **Implementation**: Proper attribution display, rate limit compliance, consideration of commercial tile providers for production
  
- **OpenStreetMap Foundation, Nominatim Usage Policy**: <https://operations.osmfoundation.org/policies/nominatim/>
  - Geocoding service usage limits, caching rules
  - **Relevance**: Legal compliance for reverse geocoding
  - **Implementation**: Caching geocoded results, respecting rate limits
  
- **W3C Geolocation API Specification**: <https://www.w3.org/TR/geolocation-API/>
  - Consent requirements, accuracy levels, error handling, position caching
  - **Relevance**: Browser/mobile standard for location access
  - **Implementation**: Explicit consent prompts, accuracy level selection, error state handling

### Child Safety and Content Moderation

- **UK Age Appropriate Design Code (Children's Code)**: <https://ico.org.uk/for-organisations/uk-gdpr-guidance-and-resources/children/childrens-code-appropriate-design-code/>
  - 15 standards for services accessed by children: best interests, DPIA, age assurance, transparency, parental controls, etc.
  - **Relevance**: Influential framework for child-safe design, even outside UK
  - **Implementation**: Age gates, parental consent flows, default privacy settings, data minimization for minors
  
- **COPPA (Children's Online Privacy Protection Act)**: <https://www.ftc.gov/business-guidance/resources/complying-coppa-frequently-asked-questions>
  - Verifiable parental consent for under-13 users, data deletion rights, limited data collection
  - **Relevance**: US federal regulation for services targeting children under 13
  - **Implementation**: Age verification, parental consent mechanism, data deletion workflows

### Incident Response and Business Continuity

- **NIST SP 800-61 Rev. 2**: <https://csrc.nist.gov/publications/detail/sp/800-61/rev-2/final>
  - Computer Security Incident Handling Guide: preparation, detection, analysis, containment, eradication, recovery, post-incident learning
  - **Relevance**: Standard incident response framework
  - **Implementation**: Incident response plan, severity classification, escalation procedures, tabletop exercises
  
- **ISO 22301**: <https://www.iso.org/standard/75106.html>
  - Business Continuity Management Systems
  - **Relevance**: Ensures service availability during disruptions
  - **Implementation**: Backup/restore procedures, failover planning, disaster recovery testing

### Accessibility

- **WCAG 2.1 Level AA**: <https://www.w3.org/WAI/WCAG21/quickref/>
  - Touch target sizes (44x44px minimum), color contrast (4.5:1), screen reader support, reduced motion, keyboard navigation
  - **Relevance**: Legal and ethical accessibility requirements
  - **Implementation**: Accessible UI components, screen reader labels, sufficient contrast ratios, touch target sizing

### Academic Research on Location Privacy

- **"Geo-Indistinguishability: Differential Privacy for Location-Based Systems"** (ACM CCS 2013): <https://www.cs.cmu.edu/~cyang/pub/ccs13_geoprivacy.pdf>
  - Andrés, M. E., Bordenabe, N. E., Palamidessi, C., & Pérez, G. L.
  - Formal privacy guarantee using planar Laplace mechanism; ε-geo-indistinguishability definition
  - **Relevance**: Rigorous mathematical framework for location privacy
  - **Implementation**: Approximate location feature uses controlled precision reduction; future work could implement formal geo-indistinguishability
  
- **"Location Privacy in the Era of Big Data"** (IEEE Security & Privacy, 2015)
  - Shokri, R., & Theodorakopoulos, G.
  - Survey of location privacy threats, k-anonymity, differential privacy applications, inference attacks
  - **Relevance**: Comprehensive overview of location privacy research landscape
  - **Implementation**: Awareness of inference attacks informs data minimization and retention policies

### Industry Best Practices

- **CNCF Security Whitepaper**: <https://github.com/cncf/tag-security/blob/main/security-whitepaper/README.md>
  - Container security, Kubernetes hardening, supply chain security, cloud-native security patterns
  - **Relevance**: Production deployment security for containerized backends
  - **Implementation**: Container image scanning, minimal base images, network policies, secrets management
  
- **CIS Benchmarks**: <https://www.cisecurity.org/benchmark>
  - Configuration hardening for PostgreSQL, Docker, Linux, Kubernetes, cloud platforms
  - **Relevance**: Industry-standard security configuration baselines
  - **Implementation**: Database hardening, OS security configurations, container runtime security

---

## Research conclusions

- **OSM tiles and Nominatim**: Suitable for prototypes and development; production deployments should evaluate commercial tile providers or self-hosted tile servers for SLA guarantees and offline capability.

- **Mobile background location**: Both Android and iOS require explicit user-facing justification, platform-specific permission declarations, just-in-time notifications, and battery-aware behavior. Background location triggers enhanced review processes on both platforms.

- **OAuth 2.0 for mobile**: Mobile clients are public OAuth clients requiring authorization code flow with PKCE (RFC 7636). Implicit grant is deprecated. Short-lived access tokens with rotating refresh tokens provide session security.

- **Web security policy**: A strict Content Security Policy, HTTPS enforcement, and secure cookie configuration matter for any web companion or admin interface. Native mobile apps use platform secure storage rather than browser cookies for token storage.

- **GDPR compliance**: Requires explicit, granular consent per sharing relationship (not blanket consent), data minimization (collect only what's needed), purpose limitation, and erasure mechanisms. All incorporated into MVP design through directional consent records, expiry, and deletion workflows.

- **OWASP MASVS and API Security Top 10**: Provide concrete, testable security controls for validation during the internship. Each control in the [Security Control Register](../reference/security-control-register.md) maps to specific MASVS requirements and OWASP API Security threats.

---

## How to use this document

This document serves multiple purposes:

1. **Validation source**: When making architectural decisions, reference the relevant standards and research to validate the approach.

2. **Compliance mapping**: Use the GDPR articles, ISO controls, and platform guidelines to demonstrate regulatory compliance in reports.

3. **Test case generation**: OWASP MASVS and API Security Top 10 provide ready-made security test scenarios.

4. **Further reading**: Links point to primary sources for deeper investigation when needed.

5. **Evidence for reviewers**: Mentor reviews and internship documentation can cite these sources to show research-backed design decisions.

---

*Record new decisions, sources, assumptions, and unresolved risks here as research progresses.*

**Cross-references:**
- [§2 Constraints](../architecture/constraints.md) — how research shaped technology constraints
- [Technology Stack and Infrastructure](../reference/technology-stack-and-infrastructure.md) — frozen stack rationale
- [Security Control Register](../reference/security-control-register.md) — SC-01 through SC-09 with test procedures
- [Privacy and Data Handling](../architecture/privacy-and-data-handling.md) — GDPR compliance details
- [Incident Response Plan](../architecture/incident-response-plan.md) — NIST SP 800-61 alignment
