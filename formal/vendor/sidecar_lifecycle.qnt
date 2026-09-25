module sidecar_lifecycle {
  type Phase = Booting | Ready | Draining | Stopped | Failed

  type State = {
    phase: Phase,
    serving: bool,
    telemetryAvailable: bool,
  }

  var s: State

  action init: bool = s' = {
    phase: Booting,
    serving: false,
    telemetryAvailable: true,
  }

  action becomeReady: bool = all {
    s.phase == Booting,
    s' = { ...s, phase: Ready, serving: true },
  }

  action telemetryFailure: bool = all {
    s.phase == Ready,
    s.telemetryAvailable,
    s' = { ...s, telemetryAvailable: false },
  }

  action beginDrain: bool = all {
    s.phase == Ready,
    s' = { ...s, phase: Draining, serving: false },
  }

  action stop: bool = all {
    s.phase == Draining,
    s' = { ...s, phase: Stopped, serving: false },
  }

  action bootFailure: bool = all {
    s.phase == Booting,
    s' = { ...s, phase: Failed, serving: false },
  }

  action stutter: bool = s' = s

  action step: bool = any {
    becomeReady,
    telemetryFailure,
    beginDrain,
    stop,
    bootFailure,
    stutter,
  }

  val safety: bool = and {
    s.serving == (s.phase == Ready),
    not(Set(Draining, Stopped, Failed).contains(s.phase)) or not(s.serving),
    s.telemetryAvailable or s.phase != Ready or s.serving,
  }

  val degraded_telemetry_still_serves: bool =
    s.phase == Ready and not(s.telemetryAvailable) and s.serving

  val stopped_reachable: bool = s.phase == Stopped
}
