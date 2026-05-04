# jobs

## Purpose

Background worker and automation runtime area for future Nexus job processing.

## Current Status

Parked/future area with scaffold code.

## What Belongs Here

- billing jobs
- claim status polling
- report generation
- migrations/imports
- FHIR synchronization
- eligibility checks
- claim batch submission

## What Does Not Belong Here

- synchronous request/response API handlers
- one-off scripts that should be deterministic app code

## Notes

Activation should follow clear milestones after backend foundations, auth/RBAC, and persistence are stabilized.
