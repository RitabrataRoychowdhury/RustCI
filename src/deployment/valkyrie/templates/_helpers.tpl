{{/*
Expand the name of the chart.
*/}}
{{- define "valkyrie-protocol.name" -}}
{{- default .Chart.Name .Values.nameOverride | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Create a default fully qualified app name.
We truncate at 63 chars because some Kubernetes name fields are limited to this (by the DNS naming spec).
If release name contains chart name it will be used as a full name.
*/}}
{{- define "valkyrie-protocol.fullname" -}}
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
{{- define "valkyrie-protocol.chart" -}}
{{- printf "%s-%s" .Chart.Name .Chart.Version | replace "+" "_" | trunc 63 | trimSuffix "-" }}
{{- end }}

{{/*
Common labels
*/}}
{{- define "valkyrie-protocol.labels" -}}
helm.sh/chart: {{ include "valkyrie-protocol.chart" . }}
{{ include "valkyrie-protocol.selectorLabels" . }}
{{- if .Chart.AppVersion }}
app.kubernetes.io/version: {{ .Chart.AppVersion | quote }}
{{- end }}
app.kubernetes.io/managed-by: {{ .Release.Service }}
app.kubernetes.io/part-of: valkyrie-protocol
{{- with .Values.commonLabels }}
{{ toYaml . }}
{{- end }}
{{- end }}

{{/*
Selector labels
*/}}
{{- define "valkyrie-protocol.selectorLabels" -}}
app.kubernetes.io/name: {{ include "valkyrie-protocol.name" . }}
app.kubernetes.io/instance: {{ .Release.Name }}
{{- end }}

{{/*
Create the name of the service account to use
*/}}
{{- define "valkyrie-protocol.serviceAccountName" -}}
{{- if .Values.serviceAccount.create }}
{{- default (include "valkyrie-protocol.fullname" .) .Values.serviceAccount.name }}
{{- else }}
{{- default "default" .Values.serviceAccount.name }}
{{- end }}
{{- end }}

{{/*
Create the name of the TLS secret to use
*/}}
{{- define "valkyrie-protocol.tlsSecretName" -}}
{{- if .Values.tls.secretName }}
{{- .Values.tls.secretName }}
{{- else }}
{{- printf "%s-tls" (include "valkyrie-protocol.fullname" .) }}
{{- end }}
{{- end }}

{{/*
Generate certificates for Valkyrie Protocol
*/}}
{{- define "valkyrie-protocol.gen-certs" -}}
{{- $altNames := list ( printf "%s.%s" (include "valkyrie-protocol.name" .) .Release.Namespace ) ( printf "%s.%s.svc" (include "valkyrie-protocol.name" .) .Release.Namespace ) -}}
{{- $ca := genCA "valkyrie-protocol-ca" 365 -}}
{{- $cert := genSignedCert ( include "valkyrie-protocol.name" . ) nil $altNames 365 $ca -}}
tls.crt: {{ $cert.Cert | b64enc }}
tls.key: {{ $cert.Key | b64enc }}
ca.crt: {{ $ca.Cert | b64enc }}
{{- end }}

{{/*
Return the proper Valkyrie Protocol image name
*/}}
{{- define "valkyrie-protocol.image" -}}
{{- $registryName := .Values.valkyrie.image.registry -}}
{{- $repositoryName := .Values.valkyrie.image.repository -}}
{{- $tag := .Values.valkyrie.image.tag | default .Chart.AppVersion | toString -}}
{{- if .Values.global.imageRegistry }}
    {{- $registryName = .Values.global.imageRegistry -}}
{{- end -}}
{{- if $registryName }}
{{- printf "%s/%s:%s" $registryName $repositoryName $tag -}}
{{- else -}}
{{- printf "%s:%s" $repositoryName $tag -}}
{{- end -}}
{{- end }}

{{/*
Return the proper Docker Image Registry Secret Names
*/}}
{{- define "valkyrie-protocol.imagePullSecrets" -}}
{{- include "common.images.pullSecrets" (dict "images" (list .Values.valkyrie.image) "global" .Values.global) -}}
{{- end }}

{{/*
Create a default fully qualified prometheus name.
*/}}
{{- define "valkyrie-protocol.prometheus.fullname" -}}
{{- printf "%s-%s" .Release.Name "prometheus" | trunc 63 | trimSuffix "-" -}}
{{- end }}

{{/*
Create a default fully qualified grafana name.
*/}}
{{- define "valkyrie-protocol.grafana.fullname" -}}
{{- printf "%s-%s" .Release.Name "grafana" | trunc 63 | trimSuffix "-" -}}
{{- end }}

{{/*
Create a default fully qualified jaeger name.
*/}}
{{- define "valkyrie-protocol.jaeger.fullname" -}}
{{- printf "%s-%s" .Release.Name "jaeger" | trunc 63 | trimSuffix "-" -}}
{{- end }}