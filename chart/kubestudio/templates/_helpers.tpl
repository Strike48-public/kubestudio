{{/*
Expand the name of the chart.
*/}}
{{- define "kubestudio.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Create a default fully qualified app name.
*/}}
{{- define "kubestudio.fullname" -}}
{{- if .Values.fullnameOverride }}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- $name := default .Chart.Name .Values.nameOverride }}
{{- if contains $name .Release.Name }}
{{- .Release.Name | trunc 63 | trimSuffix "-" }}
{{- else }}
{{- printf "%s-%s" .Release.Name $name | trunc 63 | trimSuffix "-" }}
{{- end }}
{{- end }}
{{- end }}

{{/*
Create chart name and version as used by the chart label.
*/}}
{{- define "kubestudio.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Common labels.
*/}}
{{- define "kubestudio.labels" -}}
helm.sh/chart: {{ include "kubestudio.chart" . }}
{{ include "kubestudio.selectorLabels" . }}
app.kubernetes.io/version: {{ .Values.image.tag | default .Chart.AppVersion | quote }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- end }}

{{/*
Selector labels.
*/}}
{{- define "kubestudio.selectorLabels" -}}
app.kubernetes.io/name: {{ include "kubestudio.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{/*
Service account name.
*/}}
{{- define "kubestudio.serviceAccountName" -}}
{{- if .Values.serviceAccount.create }}
{{- default (include "kubestudio.fullname" .) .Values.serviceAccount.name }}
{{- else }}
{{- default "default" .Values.serviceAccount.name }}
{{- end }}
{{- end }}

{{/*
Binary to run based on mode.
*/}}
{{- define "kubestudio.binary" -}}
{{- if eq .Values.mode "ai-enabled" -}}
ks-connector
{{- else -}}
ks-server
{{- end -}}
{{- end }}

{{/*
Build KUBECONFIG env value from mounted secret keys.
*/}}
{{- define "kubestudio.kubeconfigPaths" -}}
{{- if .Values.existingKubeConfigSecret -}}
/etc/kubestudio/kubeconfigs/config
{{- else -}}
{{- $paths := list -}}
{{- range $key, $_ := .Values.kubeconfigs -}}
{{- $paths = append $paths (printf "/etc/kubestudio/kubeconfigs/%s" $key) -}}
{{- end -}}
{{- join ":" $paths -}}
{{- end -}}
{{- end }}
