//! Bounded provider assessment before quote acceptance, body admission or execution.
use super::{
    Error,
    authority::{Action, Authority},
    authorization::Authorized,
    contract::{Context, Limits},
    handler::Assessment,
    value::{Budget, Value},
};

/// Owner-retained provisional assessment; dropping it releases any snapshot pin.
pub struct Assessed {
    pub(crate) authorized: Authorized,
    pub(crate) assessment: Assessment,
}
impl Assessed {
    /// Borrow assessed price factors without permitting execution or settlement.
    pub fn factors(&self) -> &Value {
        tracing::trace!("Borrowing bounded provider pricing factors");
        &self.assessment.factors
    }
    /// Borrow the authorized selected terms behind this assessment.
    pub fn authorized(&self) -> &Authorized {
        tracing::trace!("Borrowing assessed invocation authority binding");
        &self.authorized
    }
    /// Read the assessed maximum complete input byte length.
    pub fn input_bytes(&self) -> u64 {
        tracing::trace!("Reading provider-assessed input bound");
        self.assessment.input_bytes
    }
}
/// Assess bounded metadata with a pinned installed provider; never execute work or reserve money.
/// # Errors
/// Rejects missing provider, role/authority failures, excessive metadata/factors/pins and family refusal.
pub fn assess(
    context: &Context<'_>,
    authority: &Authority,
    authorized: Authorized,
    metadata: &Value,
    limits: Limits,
) -> Result<Assessed, Error> {
    tracing::trace!("Assessing service metadata before accepting large input");
    limits.validate()?;
    let local = authorized.peer.local;
    let (requester, provider, _) = authorized.selection.roles();
    if local != provider {
        return Err(Error::Denied);
    }
    let action = if requester == provider {
        Action::Request
    } else {
        Action::Invoke
    };
    let budget = Budget {
        depth: 31,
        nodes: limits.nodes,
        bytes: limits.state_bytes,
    };
    let bytes = metadata.byte_len(budget)?;
    if metadata.body_count(budget)? > limits.bodies {
        return Err(Error::Limit);
    }
    context.check(authority, local, action, bytes)?;
    let assessment = authorized.implementation.provider()?.assess(
        authorized.peer,
        authorized.selection.offering(),
        metadata,
    )?;
    retain(context, authority, authorized, assessment, limits, action)
}

/// Validate a trusted remote provider assessment for an explicitly authorized local requester.
/// # Errors
/// Rejects wrong roles, local pins in remote data, fabricated input, unsupported factors and finite limits.
pub fn remote(
    context: &Context<'_>,
    authority: &Authority,
    authorized: Authorized,
    assessment: Assessment,
    limits: Limits,
) -> Result<Assessed, Error> {
    tracing::trace!("Validating remote assessment without installing a local provider");
    limits.validate()?;
    let local = authorized.peer.local;
    let (requester, provider, _) = authorized.selection.roles();
    if local != requester || local == provider || assessment.pin.is_some() {
        return Err(Error::Denied);
    }
    let budget = Budget {
        depth: 31,
        nodes: limits.nodes,
        bytes: limits.state_bytes,
    };
    let bytes = assessment
        .metadata
        .byte_len(budget)?
        .checked_add(assessment.factors.byte_len(budget)?)
        .ok_or(Error::Limit)?;
    if bytes > limits.state_bytes || assessment.input_bytes > limits.state_bytes as u64 {
        return Err(Error::Limit);
    }
    context.check(authority, local, Action::Request, bytes)?;
    authorized.implementation.requester().validate_assessment(
        authorized.input.as_ref().ok_or(Error::Denied)?,
        &assessment.metadata,
        &assessment.factors,
        budget,
    )?;
    Ok(Assessed {
        authorized,
        assessment,
    })
}

/// Assess explicit unresolved metadata before body transfer using the pinned provider's public contract.
/// # Errors
/// Wrong local role/authority, declared capacity, unsupported body semantics and dependency failures reject before work.
pub fn assess_metadata(
    context: &Context<'_>,
    authority: &Authority,
    authorized: Authorized,
    metadata: &super::input::Input,
    limits: Limits,
) -> Result<Assessed, Error> {
    tracing::trace!("Assessing bounded unresolved input through installed family ports");
    if let super::input::Input::Complete(value) = metadata {
        return assess(context, authority, authorized, value, limits);
    }
    limits.validate()?;
    let local = authorized.peer.local;
    let (requester, provider, _) = authorized.selection.roles();
    if local != provider {
        return Err(Error::Denied);
    }
    let action = if requester == provider {
        Action::Request
    } else {
        Action::Invoke
    };
    let budget = Budget {
        depth: 31,
        nodes: limits.nodes,
        bytes: limits.state_bytes,
    };
    let bytes = metadata.bounded(budget)?;
    if metadata.body_count(budget)? > limits.bodies {
        return Err(Error::Limit);
    }
    context.check(authority, local, action, bytes)?;
    let assessment = authorized.implementation.provider()?.assess_metadata(
        authorized.peer,
        authorized.selection.offering(),
        metadata,
        budget,
    )?;
    retain(context, authority, authorized, assessment, limits, action)
}

// Bound and retain local provider metadata/factors/pins only after authority and declared input admission.
fn retain(
    context: &Context<'_>,
    authority: &Authority,
    authorized: Authorized,
    assessment: super::handler::Assessment,
    limits: Limits,
    action: Action,
) -> Result<Assessed, Error> {
    tracing::trace!("Retaining bounded authorized provider assessment");
    let local = authorized.peer.local;
    let budget = Budget {
        depth: 31,
        nodes: limits.nodes,
        bytes: limits.state_bytes,
    };
    let bytes = assessment
        .metadata
        .byte_len(budget)?
        .checked_add(assessment.factors.byte_len(budget)?)
        .and_then(|n| n.checked_add(assessment.pin.as_ref().map_or(0, |b| b.bytes().len())))
        .ok_or(Error::Limit)?;
    if bytes > limits.state_bytes || assessment.input_bytes > limits.state_bytes as u64 {
        return Err(Error::Limit);
    }
    context.check(authority, local, action, bytes)?;
    Ok(Assessed {
        authorized,
        assessment,
    })
}
