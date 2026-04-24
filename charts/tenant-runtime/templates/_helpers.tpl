{{- define "tenant-runtime.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" -}}
{{- end -}}

{{- define "tenant-runtime.fullname" -}}
{{- if .Values.fullnameOverride -}}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" -}}
{{- else -}}
{{- printf "%s-%s" .Release.Name (include "tenant-runtime.name" .) | trunc 63 | trimSuffix "-" -}}
{{- end -}}
{{- end -}}

{{- define "tenant-runtime.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" -}}
{{- end -}}

{{- define "tenant-runtime.labels" -}}
helm.sh/chart: {{ include "tenant-runtime.chart" . }}
app.kubernetes.io/name: {{ include "tenant-runtime.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
app.kubernetes.io/component: tenant-runtime
app.kubernetes.io/part-of: hermes
hermes.io/tenant-id: {{ .Values.tenant.id | quote }}
hermes.io/workspace-id: {{ .Values.tenant.workspaceId | quote }}
hermes.io/plane: {{ .Values.tenant.plane | quote }}
{{- end -}}

{{- define "tenant-runtime.selectorLabels" -}}
app.kubernetes.io/name: {{ include "tenant-runtime.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end -}}

{{- define "tenant-runtime.serviceAccountName" -}}
{{- if .Values.serviceAccount.create -}}
{{- default (include "tenant-runtime.fullname" .) .Values.serviceAccount.name -}}
{{- else -}}
{{- default "default" .Values.serviceAccount.name -}}
{{- end -}}
{{- end -}}

{{- define "tenant-runtime.configMapName" -}}
{{- if .Values.config.existingConfigMap -}}
{{- .Values.config.existingConfigMap -}}
{{- else -}}
{{- printf "%s-config" (include "tenant-runtime.fullname" .) -}}
{{- end -}}
{{- end -}}

{{- define "tenant-runtime.runtimeSecretName" -}}
{{- if .Values.runtimeSecret.name -}}
{{- .Values.runtimeSecret.name -}}
{{- else -}}
{{- printf "%s-runtime" (include "tenant-runtime.fullname" .) -}}
{{- end -}}
{{- end -}}

{{- define "tenant-runtime.pvcName" -}}
{{- if .Values.knowledgeBase.existingClaim -}}
{{- .Values.knowledgeBase.existingClaim -}}
{{- else -}}
{{- printf "%s-kb" (include "tenant-runtime.fullname" .) -}}
{{- end -}}
{{- end -}}
