{{- define "cyberepi.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" -}}
{{- end -}}

{{- define "cyberepi.fullname" -}}
{{- if .Values.fullnameOverride -}}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" -}}
{{- else -}}
{{- printf "%s" (include "cyberepi.name" .) | trunc 63 | trimSuffix "-" -}}
{{- end -}}
{{- end -}}

{{- define "cyberepi.image" -}}
{{- printf "%s/%s:%s" .Values.image.registry .Values.image.repository (.Values.image.tag | default .Chart.AppVersion) -}}
{{- end -}}

{{- define "cyberepi.webImage" -}}
{{- printf "%s/%s:%s" .Values.image.registry .Values.web.repository (.Values.image.tag | default .Chart.AppVersion) -}}
{{- end -}}

{{- define "cyberepi.security" -}}
runAsNonRoot: true
runAsUser: 65532
runAsGroup: 65532
seccompProfile:
  type: RuntimeDefault
{{- end -}}

{{- define "cyberepi.containerSecurity" -}}
allowPrivilegeEscalation: false
readOnlyRootFilesystem: {{ .Values.podSecurity.readOnlyRootFilesystem }}
capabilities:
  drop: ["ALL"]
{{- end -}}
