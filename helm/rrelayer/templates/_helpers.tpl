{{- define "rrelayer.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" -}}
{{- end -}}

{{- define "rrelayer.fullname" -}}
{{- if .Values.fullnameOverride -}}
{{- .Values.fullnameOverride | trunc 63 | trimSuffix "-" -}}
{{- else -}}
{{- $name := default .Chart.Name .Values.nameOverride -}}
{{- printf "%s-%s" $name .Release.Name | trunc 63 | trimSuffix "-" -}}
{{- end -}}
{{- end -}}

{{- define "rrelayer.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" -}}
{{- end -}}

{{- define "rrelayer.labels" -}}
app.kubernetes.io/name: {{ include "rrelayer.name" . }}
helm.sh/chart: {{ include "rrelayer.chart" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
{{- with .Values.podLabels }}
{{ toYaml . | indent 4 }}
{{- end }}
{{- end -}}

{{- define "rrelayer.selectorLabels" -}}
app.kubernetes.io/name: {{ include "rrelayer.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end -}}
